+++
title = "Making the stack for my emulator"
date = 2026-09-22
[taxonomies]
tags = ["rust", "emulation"]
+++

See thing is, making the stack is as easy as giving it an address, usually the end of the entire memory region. in my case at `0x0800_0000` (128MiB in), because it grows negatively inwards. And to use it, all you have to do, is move around the `sp` register (stack pointer) which is the alias for `x2`. and then calling sw and lw on this register to do pushing and popping operations!

no need to set its location at the start of the program though, it is preemptively set:
```rust
pub fn new(bus: Bus) -> Self {
    let mut cpu = Self {
        regs: [0; 32],
        pc: DRAM_BASE,
        bus,
    };
    // set stack pointer (sp) to somewhere at the top of the memory
    cpu.regs[2] = 0x0800_0000; 
    cpu
}
```

we can actually test it right now, I'll explain what this does in the next post, but here's the test:
```asm
.section .text
.globl _start
_start:
    mv s0, sp # save original sp

    
    # push two values (LIFO order)
    li a0, 1
    li t0, 0xAAAAAAAA
    addi sp, sp, -4
    sw t0, 0(sp)

    li a0, 2
    li t1, 0xBBBBBBBB
    addi sp, sp, -4
    sw t1, 0(sp)

    # pop top, verify it's t1
    li a0, 3
    lw t2, 0(sp)
    bne t2, t1, fail
    addi sp, sp, 4

    # pop next, verify it's t0
    li a0, 4
    lw t2, 0(sp)
    bne t2, t0, fail
    addi sp, sp, 4

    # sp must be back where we started
    li a0, 5
    bne sp, s0, fail

    # nested call: callee saves ra/s0/s1, uses its own frame
    li a0, 6
    jal ra, inner
    li t3, 123
    bne a0, t3, fail

    li a0, 0 # success
    j done
fail:
done:
    li a7, 93
    ecall

inner:
    addi sp, sp, -16 # make a frame
    sw ra, 12(sp)
    sw s0, 8(sp)
    sw s1, 4(sp)

    li s1, 123
    mv a0, s1

    lw s1, 4(sp)
    lw s0, 8(sp)
    lw ra, 12(sp)
    addi sp, sp, 16
    ret
```

this simple stack test passed.
> Meaning we got exit code 0 after executing it, due to exit being called while `a0` had `0x0` loaded and not `0x1`, `0x2`, `0x3`, `0x4`, `0x5` or `0x6`, as these were the fail cases inside the code, as you can see from all the li a0, n calls.

This means we are ready for stack frames for all our functions, It is however the job of the codegen and by extention the compiler to generate all the assembly for the function prologue and epilogue* which I'll be talm bout soon

The function prologue/epilogue are headers and footers for our functions, which need to facilitate the creation of the stack frame and storing the return address and other information, at the start.
and then pulling back the stack pointer and reloading the previous return address from the memory at the end.
