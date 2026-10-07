+++
title = "Emulation Journey"
date = 2026-09-19
[taxonomies]
tags = ["rust", "emulation"]
+++

I decided to make an emulator alongside my programming language which within itself is a separate and very cool project! I get to understand and implement the ISA's scientists and engineers have carefully engineered

I'm going with the RISC-V ISA, specifically the RV32IM (possibly RV32IMFD). this allows native operations on 32 bit integers alongside multiplication and division! and th3 "possibly" one also adds the single and double precision floating point extentions!

{{ image(url="public/isas.jpg", alt="Incase you want to learn more about other extentions ") }}

## Preface
this ISA is PACKED man. and there are some core principles that need to be upheld that make this packedness property extremely interesring. 

so for starters, each instruction consists of an opcode (7 bits) and possibly 2 other descriptors for it (funct3 which is 3 bits and funct7 which is 7 bits)

there's also always the rd register number, and the rs1 and rs2 registers for instructions that do need them. since there are 32 registers, you KNOW they only use 5 bits to represent each.

so overall for the easiest format of instruction we'll learn today (the R-type) which needs every single one of these, we have everything ready! R-types are the register + register instructions

example:
```asm
add x1, x2, x3 # x1 = x2 + x3
```
the I-types are another easy one we could quickly glance past! it's just register + immediate (constants) operations

and decoding them actually needs some skill, when it comes to retrieving the immediate (constant):
```rust
fn decode_i_imm(inst: u32) -> u32 {
    sign_extend(inst >> 20, 12)
}
```
the constants we're allowed to use in I-types are 12 bit integers. we shift the number 20 to the right and then extend the two's complement signed 12-bit integer out of it

examples of the I-type could be immediate binary ops between rs1 and them which get saved to rd:

```asm
addi x1, x2, 50 # x1 = x2 + 50
```
brace yourself because this concludes the easy formats. the J-type and B-type for jumping and branching (conditional jumps and control flow) get EXTRA confusing because of how packed this thing is

## Other instruction types

the S-type is for store, and it utilizes both source registers (rs1 = base address and rs2 = value to store)
examples:
```asm
sw x1, 8(x2) # mem[x2 + 8] = x1
```
now comes the time to talk about this one rule that neeeds to be upheld: keeping rs1, rs2, rd, funct3 and opcode rigidly in place (in the same bit positions) across as many formats as possible, so that the CPU and the hardware have an easier time accessing these bits. 

So there are these 3 formats that cannot pack their desired n-bit signed immediate integer (their constant operand) all in one piece,  the S-type that we just talked about, the J-type and the B-type. instead they scatter the immediate number all over the instruction! which is really really cool.

after i talk about what instructions include the B-type and J-type, we can look at the most extreme version of this immediate number scattering which happens in the B-type

examples of the J-type:
```asm
# pc is the program counter, jal is the unconditional jump opcode

jal x1, label # x1 = pc + 4 (return address), pc += offset (jump to a new offset)

jal x0, label # the x0 register is always 0, so this just jumps to a label without a care about the return addr
and the B-type:
beq x1, x2, label # if x1 == x2 -> pc += offset
```
this might JUMP at you why the B-type would be the most extreme one, it is because it involves both source registers AND an immediate number, and also funct3 descriptors are allowed for variety in the opcodes, and this immediate number is a 13-bit one scattered all over the instruction, and can be retrieved like so:

```rust
fn decode_b_imm(inst: u32) -> u32 {
    let imm = (((inst >> 31) & 0x1) << 12)
            | (((inst >> 7)  & 0x1) << 11)
            | (((inst >> 25) & 0x3f) << 5)
            | (((inst >> 8)  & 0xf) << 1);
    sign_extend(imm, 13)
}
```

this translates to the following mapping for the immediate number in the B-type operations being scattered like so:
```
bit 31 -> imm[12]
bit 7 -> imm[11]
bit 30..25 -> imm[10:5]
bit 11..8 -> imm[4:1]
bit 0 -> imm[0]
```
and then we can reconstruct the immediate as a two's complement 13-bit signed integer and start execution of our conditional jumps! (`beq`, `bne`, `blt`*, `bge` `etc`)

> Also a side note, there are the BLT and BGE branch operations which are "less than" and "greater than or equal to", but there are no "less than or equal to" or "greater than" ops, because these 2 ops are mutually exclusive! (a is either less than b, or it is necessarily greater than or equal to b), it goes without saying, but im only saying this because my language along pretty much all of them out there have all 4 permutations of this, so this needs to be accounted for

## Rust song and dance
I swear I'll stop touching on very surface level stuff and start researching the actual logistics of this but this is another cool thing: a little Rust song and dance when it comes to the load instructions (they are I-types), as we try to read from the RAM (this is effectuated by the virtual bus)

```rust
// in the opcode matching
LOAD => {
    let imm = decode_i_imm(inst);
    let addr = self.regs[rs1].wrapping_add(imm);
    let value = match funct3 {
        0x0 => self.bus.read_byte(addr) as i8 as i32 as u32, // LB
        0x1 => self.bus.read_half(addr) as i16 as i32 as u32, // LH
        // ... other possible loading modes (Lw, LBU, LHU)
    };

    self.write_reg(rd, value);
}
```
only interested in the LB (load byte) and LH (load half) operations here, because 1. they are signed integers and 2. they are not quite a word! and the combination of these 2 facts, can get us doing something as bizarre as this:
```rust
read_byte(addr) as i8 as i32 as u32
```
Three type casts in a row, but it makes perfect sense. it comes back to the two's complement situation and it also kinda reminds me of shifting gears in a car. 

the first thing to point out is that, crudely speaking, unsignedness is both useful when representing unsigned integers, and also for raw information as well, so no disrespect to whoever requested the LB operation, the bus can only give us raw bytes, which come in the form of `u8`. then we can assign it to actually it being an `i8`, and the important insight here is that, at the end we want to treat it as raw information, but don't go out there casting the `i8` to a `u32`, that's 2 little requests at once. this is the shifting gears analogy i came up with. first extend the integer to 32-bit signed (`as i32`) and then and only then turn it to crude information! (`as u32`)

or you can use this cool function I use when the conversions aren't as easy as 8/16 to 32 :

```rust
#[inline]
fn sign_extend(value: u32, bits: u32) -> u32 {
    let shift = 32 - bits;
    ((value << shift) as i32 >> shift) as u32
}
```

this concludes the very surface level stuff I wanted to first talk about, next on the list is adding syscalls! through ecall ops and a Linux ABI calling convention that I'll happily copy so I can immediately test it out using `riscv32-unknown-elf-gcc`

## Execution
the program counter (`pc`) is the address representing the current instruction being executed in memory. it is initially set to the constant `DRAM_BASE`:
```rust
const DRAM_BASE: u32 = 0x0000_0000;
```
our driver code for the emulator, being a simple one, is expecting a .bin file which just contains all the instructions and data with no header, not an ELF which has those, so not only do we compile the program into elf, we also have to objcopy and extract the binary out of the elf!

here's how we do exactly that:
```bash
riscv64-unknown-elf-gcc -nostdlib -nostartfiles -march=rv32i -mabi=ilp32 -Ttext=0x0 -o hello.elf hello.s

riscv64-unknown-elf-objcopy -O binary hello.elf hello.bin
```
our emulator will then start writing the .bin file word for word (lol) in the memory starting from DRAM_BASE (0u32), beginning from the .text at the 0th byte, as per our instructions.

Peep the argument -Ttext=0x0. this should hopefully make the ELF linker put .text at address 0, matching our DRAM_BASE and ultimately the initial program counter (pc). so we can begin execution by executing each instruction in series and moving forward

the key here is just making sure the emulator's pc starts where the binary expects

Once I add ecalls according to the Linux ABI calling convention, hello world should look like this:

```asm
.section .data
msg:
    .asciz "Hello from my emulator!\n"

.section .text
.globl _start
_start:
    # syscall function ids are loaded into the a7 register, and their arguments are loaded into a0 and onwards
    li a7, 64  # "write" syscall
    li a0, 1   # stdout for arg0 of "write"
    la a1, msg # address for the start of our string 
    li a2, 24 # length of our string
    ecall

    li a7, 93  # exit syscall
    li a0, 0 # exit code 0 
    ecall
```