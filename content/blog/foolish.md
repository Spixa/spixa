+++
title = "Forgetting to set up crt0"
date = 2026-09-26
[taxonomies]
tags = ["rust", "emulation", "compiler"]
+++

Foolish was I, for not having a properly set up [crt0](https://en.wikipedia.org/wiki/Crt0) before writing C code. 

We actually don't need the usual things you'd think the `crt0` does like calling main, since this is the crt0 for the runtime itself.

But you NEED to set up your global pointer, since it is heavily used by compiler optimizations to save an instruction when it comes to traversing around the `.sbss` and `.sdata` section. otherwise you'll read garbage and unfortunately it was IMPOSSIBLE for me to figure out what exactly was wrong on my own.

By the way, other `crt0` procedures other than setting up argc and argv and calling main, include setting sp to stack top, which [we already did](@/blog/stack.md). and zeroing out the `.bss` section. which we don't need to do since
1. I allocate my DRAM as a `vec![0; 128 MiB]` to begin with
2. We're probably gonna avoid having anything at `.bss` anyway

{% alert(important = true) %}
#### About the global pointer
a single 12-bit signed integer is used relative to the gp register that can look ±2KiB around, it is primarily used as optimization since it occupies a register and doesn't require loading anything. 
Two other approaches of course are using the program counter, which is a full 32-bit integer, allowing us to jump around ±2GiB:
```asm
auipc a0, 0x1        # a0 = pc + 0x1000
addi  a0, a0, 0x24   # a0 = pc + 0x1024
```
And also the absolute way:
```asm
lui  a0, 0x10000     # a0 = 0x10000000
addi a0, a0, 0x24    # a0 = 0x10000024
```
Which has its own quirks the C compiler handles, but for "small data" the compilers always prefers the gp-relative way which is only 1 instruction:
```asm
lw a0, -2034(gp)     # a0 = *(gp - 2034)
```
we can set up our global pointer, using the linker, which points gp into the middle of our small data cluster, allowing us to maximize our ±2KiB range. or we can just disable it for now using `-msmall-data-limit=0`

{% end %}