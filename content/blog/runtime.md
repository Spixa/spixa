+++
title = "Emulator runtime"
date = 2026-09-26
[taxonomies]
tags = ["rust", "emulation", "langdev"]
+++

I can now run and execute C code, so I made a C runtime for my emulator! Nothing too crazy but now an actual "language" like C can work with my emulator now which is cool. the runtime gets linked against all the assembly for testing:

```bash
riscv64-elf-gcc -march=rv32im -mabi=ilp32 -ffreestanding 
    -nostdlib -nostartfiles -O2 -Wall -Wextra 
    -o elf/test_heap.elf src/asm/test_heap.s build/runtime.o 
    -Wl,-Ttext=0x0 -nostdlib -nostartfiles -lgcc
```

## Calling syscalls
Here we have a couple of examples of the C runtime taking arguments and using our syscalls to do various stuff:
```c
static inline long sys_brk(unsigned long new_break) {
    register long a0 asm("a0") = (long) new_break; // arg0 is new_break
    register long a7 asm("a7") = (long) 214; // sysbrk 
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

static inline long sys_write(int fd, const void *buf, unsigned long n) {
    register long a0 asm("a0") = fd;
    register long a1 asm("a1") = (long) buf;
    register long a2 asm("a2") = (long) n;
    register long a7 asm("a7") = 64;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}
```

But wait, what is `sys_brk`? Well of course, it calls the `brk` syscall, setting the program break, to `new_break`. Or if the given `new_break` is `0`, it queries current break. What is it useful for? Making an allocator!

Here's the `sys_brk` implementation in the emulator as a syscall:
```rust
/// args: a0=new break (0 = query)
/// ret: a0 = 0 -> success, others failure
fn sys_brk(&mut self) {
    let new_break = self.regs[10];

    if new_break == 0 {
        self.regs[10] = self.program_break; // query
        return;
    }

    if !(HEAP_BASE..MMAP_BASE).contains(&new_break) || (new_break & 0x3) != 0 {
        self.regs[10] = self.program_break; // failure + query return
        return;
    }

    self.program_break = new_break;
    self.regs[10] = 0; // success
}
```

## Freestanding prerequisites 
Also for any future runtime additions, we must provide the basic memory functions since they are omitted due to `-ffreestanding`:
```c
void *memcpy(void *dst, const void *src, size_t n) {
    char *d = (char*) dst;
    const char *s = (const char* )src;
    for (size_t i = 0; i < n; i++) d[i] = s[i];
    return dst;
}

void *memset(void *dst, int c, size_t n) {
    char *d = (char*) dst;
    for (size_t i = 0; i < n; i++) d[i] = (char)c;
    return dst;
}

void *memmove(void *dst, const void *src, size_t n) {
    char *d = (char* )dst;
    const char *s = (const char*) src;
    if (d < s) {
        for (size_t i = 0; i < n; i++) d[i] = s[i];
    } else if (d > s) {
        for (size_t i = n; i > 0; i--) d[i-1] = s[i-1];
    }
    return dst;
}

int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *x = (const unsigned char*) a;
    const unsigned char *y = (const unsigned char*) b;
    for (size_t i = 0; i < n; i++) {
        if (x[i] != y[i]) return (int)x[i] - (int)y[i];
    }
    return 0;
}
```
