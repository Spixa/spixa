+++
title = "The allocator"
date = 2026-09-27
[taxonomies]
tags = ["rust", "emulation"]
+++

I have made a free list allocator! But first I need to address a funny problem I came across, which was not setting up a proper `crt0` before proceeding to write C code! So naive of me. You can read more about it on [here](@/blog/foolish.md).

With that out of the way, we can define our global constants without needing to worry about compiler optimizations destroying things

## Free list allocator
> A free list (or freelist) is a data structure used in a scheme for dynamic memory allocation. It operates by connecting unallocated regions of memory together in a linked list, using the first word of each unallocated region as a pointer to the next. It is most suitable for allocating from a memory pool, where all objects have the same size. - Wikipedia

> Free lists make the allocation and deallocation operations very simple. To free a region, one would just link it to the free list. To allocate a region, one would simply remove a single region from the end of the free list and use it. If the regions are variable-sized, one may have to search for a region of large enough size, which can be expensive. - Wikipedia

The free list is a doubly linked list of the `BlockHeader` type that we will define. The free list is populated during the `__free` routine, where we will push the freed block into the free list. The freed blocks are kept in sequence inside the linked list.

Once we want to allocate memory by calling, `__alloc`, we always first look into our free list, because our freed blocks can be perfect for our new allocation. If such is the case, they will be popped from the free list, and reused by our new memory allocation.

There's also the concept of memory coalescing. Where if we have a free call that is surrounded by one or more free blocks *around* it (meaning on its left and right), we will merged all of them into one, and push the entire new block into the free list. 

## The setup
Our global constants, now that we can have them in `.sbss` and `.sdata`:
```c
static char* g_brk = 0; // cached program break
static char* g_heap_start = 0;
static int g_heap_ready = 0; // boolean
static BlockHeader* g_free_list; // parent node of our linked list
```

We will be utilizing the newly added program break from the emulator to do our memory allocation. I added it back in [this post](@/blog/runtime.md).

## Basic structure
a heap block header sits before the user pointer. The block header layout is 16 bytes, and in essence, it is a doubly linked list:
```c
typedef struct BlockHeader {
    size_t size_flags;
    struct BlockHeader* prev_free;
    struct BlockHeader* next_free;
    uint32_t mark_bits; // for gc later on
} BlockHeader;
```

We also define some useful constants and macros for ease, which will be explained soon:
```c
#define FLAG_IN_USE 0x1u // allocated to user code
#define FLAG_PINNED 0x2u // TODO: reserved, means dont relocate during GC
#define FLAG_MASK   0x7u

#define HDR_SIZE    16u
#define FTR_SIZE    4u
#define OVERHEAD    (HDR_SIZE + FTR_SIZE)
#define MIN_BLOCK   24u
#define ALIGN8(n)   (((n) + 7u) & ~(size_t)7u)
```

## Helper functions
This section heavily relies on boolean arithmetics for the aligning and flag writing/reading, so hopefully you are equipped with this knowledge

The size flag, being a `size_t`, is a 32-bit integer. But since our memory is 8-byte aligned, it is guaranteed that the first 3 bits are zeroed. So they contain no useful information, meaning we can use those 3 bits as flags. For now there is only 1 useful flag we care about: `FLAG_IN_USE`

Here is how we extract the size and whether or not the block is in use, from the header:
```c
static inline size_t blk_size(BlockHeader* h) { 
    return h->size_flags & ~FLAG_MASK; 
}

static inline int blk_in_use(BlockHeader* h) { 
    return h->size_flags & FLAG_IN_USE; 
}
```
{% alert(note = true) %}
`FLAG_MASK` is `111` in binary, meaning it is 29 `0`s and then 3 `1`s, so if we do a bitwise `&` of its complement (which is 29 `1`s and 3 `0`s), we end up just having the first 29 bits, giving us the actual size of the block, which is a number divisble by 8, due to it being 8-aligned in nature and the last 3 bits being `0`.
{% end %}

Using some more boolean arithmetics, we can also set the size and flags as well:

```c
static inline void blk_set_size(BlockHeader* h, size_t s) {
    h->size_flags = s | (h->size_flags & FLAG_MASK);
}

static inline void blk_set_flags(BlockHeader* h, uint32_t f) {
    h->size_flags = (h->size_flags & ~FLAG_MASK) | (f & FLAG_MASK);
}
```

### Setting up the heap
A helper function will set the heap up for us. The way we do this is, by caching the current system break the emulator will provide us with, by calling `sys_brk(0)` which was the query directive.

```c
static void heap_ensure_init() {
    if (g_heap_ready) return;
    long cur = sys_brk(0);
    g_brk = (char*)(cur > 0 ? cur : 0);
    
    debug_str("heap initialized");
    g_heap_start = g_brk;
    g_heap_ready = 1;
}
```

## Implementing `__alloc(size_t n)`

### Initializing heap
We first ensure the heap is initialized, and then 8-byte align our needed amount of space + the overhead the header introduces. Since we want to do coalescing, there is a minimum block size which we also need to ensure. 
```c
heap_ensure_init();
if (!g_brk) return 0; // fail

if (size == 0) size = 1;
size_t need = ALIGN8(size + OVERHEAD);
if (need < MIN_BLOCK) need = MIN_BLOCK;

// ...
```

### Looking for free memory inside our free list
Then, we look up in our free list which was the doubly linked list we talked about. We are looking for blocks that are bigger than our need:

```c
// ...

// first look in the free list and see if anything fits
for (BlockHeader* h = g_free_list; h; h = h->next_free) {
    if (blk_size(h) >= need) {
        free_list_remove(h);
        blk_set_flags(h, FLAG_IN_USE);
        h->mark_bits = 0;
        set_footer(h);
        debug_str("reused free block");
        return (char*) h + HDR_SIZE;
    }
}

// ...
```

#### The footer and the `set_footer(BlockHeader*)` function
The set footer function, must be called whenever the block size changes, so the next physical block in the linked list can find us:
```c
static inline void set_footer(BlockHeader* h) {
    *(uint32_t*)((char*) h + blk_size(h) - FTR_SIZE) =
        (uint32_t) blk_size(h);
}
```

Using the footer, we can actually find the size of the previous block:
```c
static inline size_t prev_blk_size(BlockHeader* h) {
    return *(uint32_t*)((char*) h - FTR_SIZE);
}
```

### Looking for free memory elsewhere
Anyway, if we didn't find anything in the free list that met our needs for being replaced by our new block of memory, we need to allocate memory, by asking the emulator to provide us with a system break for us to base off the memory address from:

```c
char* old = g_brk;
char* neu = old + need;
if ((unsigned long) neu > HEAP_LIMIT) return 0; // OOM
if (sys_brk((unsigned long) neu) != 0) return 0;
g_brk = neu;

BlockHeader* h = (BlockHeader*) old;
h->size_flags = need | FLAG_IN_USE;
h->next_free = 0;
h->prev_free = 0;
h->mark_bits = 0;
set_footer(h);
debug_str("bumped brk");

return (char*) h + HDR_SIZE;
```

## Implementing `__realloc(void*, size_t)`
It's pretty easy, so I'll just drop the entire code:
```c
void *__realloc(void *ptr, size_t new_size) {
    if (!ptr) return __alloc(new_size);
    if (new_size == 0) { __free(ptr); return 0; }

    BlockHeader* h = (BlockHeader*)((char*) ptr - HDR_SIZE);
    size_t old_user_size = blk_size(h) - OVERHEAD;
    if (old_user_size >= new_size) return ptr;

    void *np = __alloc(new_size);
    if (!np) return 0;

    char* dst = (char*) np;
    char* src = (char*) ptr;
    for (size_t i = 0; i < old_user_size; i++) dst[i] = src[i];

    __free(ptr);
    return np;
}
```

## Implementing `__free(void* ptr)`
This is the most important part, because of the coalescing. 

### Marking memory block as free
First we retrieve the block header, and make sure it is not in use. Then we mark it as free by setting it's flags to `0`:
```c
if (!ptr) return;
BlockHeader* h = (BlockHeader*)((char*) ptr - HDR_SIZE);
if (!blk_in_use(h)) return; // double-free guard
blk_set_flags(h, 0); // mark free

// ...
```

### Coalescing
Now comes the merging part from either side:
```c
// merge with next physical block if free
char *next_addr = (char*) h + blk_size(h);
if (next_addr < g_brk) {
    BlockHeader* next = (BlockHeader*) next_addr;

    if (!blk_in_use(next)) {
        free_list_remove(next);
        blk_set_size(h, blk_size(h) + blk_size(next));
    }
}

// merge with previous physical block if free
if ((char*) h > g_heap_start) {
    size_t prev_size = prev_blk_size(h);
    BlockHeader* prev = (BlockHeader*)((char*) h - prev_size);
    if (!blk_in_use(prev)) {
        free_list_remove(prev); // ! important
        blk_set_size(prev, blk_size(prev) + blk_size(h));
        h = prev;
    }
}

set_footer(h); // updates size in the footer
```

{% alert(important = true) %} 
It's important we remove the blocks we are merging it with from the free list, because this new fully merged block of memory with its possibly new head and size, is going to be the one pushed into our free list:
```c
free_list_push(h);
```
{% end %}

## Implementing `__calloc(size_t n, size_t size)`
This ones also a given
```c
void *__calloc(size_t n, size_t size) {
    size_t total = n * size;
    void *p = __alloc(total);
    if (!p) return 0;
    char *c = (char*) p;
    for (size_t i = 0; i < total; i++) c[i] = 0;
    return p;
}
```

## Example usage
```asm
.section .text
.globl _start
_start:
    # three 32-byte allocations
    li s2, 1
    li a0, 32
    jal __alloc
    beqz a0, fail
    mv s0, a0   # A

    li s2, 2
    li a0, 32
    jal __alloc
    beqz a0, fail
    mv s1, a0   # B

    li s2, 3
    li a0, 32
    jal __alloc
    beqz a0, fail
    mv s3, a0   # C

    # free B, then alloc 32: must reuse B
    li s2, 4
    mv a0, s1
    jal __free

    li s2, 5
    li a0, 32
    jal __alloc
    bne a0, s1, fail

    # free the reused B and A. now A+B is a coalesced free region.
    li s2, 6
    mv a0, s1
    jal __free

    li s2, 7
    mv a0, s0
    jal __free

    # allocate 80 bytes: 80 + 20 = 100, rounded to 104 (8-byte aligned)
    # A (48) + B (48) = 96 coalesced, so a fit requires splitting
    # 96 < 104 -> must bump brk. so this does NOT reuse A.
    # allocate 64 instead: 64 + 20 = 84 -[8 byte align] -> 88: fits in 96

    li s2, 8
    li a0, 64
    jal __alloc
    bne a0, s0, fail    # must start at A

    # double free must not corrupt
    li s2, 9
    mv a0, s1
    jal __free
    mv a0, s1
    jal __free

    li a0, 0
    j done
fail:
    mv a0, s2
done:
    li a7, 93
    ecall
```

Compiling and executing this by the emulator gives us the following output:
{% crt() %}
```
heap initialized
bumped brk
bumped brk
bumped brk
freed (coalesced)
reused free block
freed (coalesced)
freed (coalesced)
reused free block

[emu] exited with code 0
```
{% end %}