+++
title = "Adding syscalls to my emulator"
date = 2026-09-21
+++

When it comes to a 32-bit Linux program, the ecall instruction triggers a system call with these register roles:
| Register      | Role  |
| -----------   | -----------|
| `a7`          | Syscall number|
| `a0-a5`       | Syscall arguments (up to 6)|
| `a0`          | Return value after call

Also it's important to note that unused argument registers do not need to be zeroed. That's just a newlib artifact. Linux itself only reads the registers it needs for a given syscall.

with that in mind I did the bare minimum and handled the `exit` and `sys_write` ecalls like so:
```rust
fn handle_ecall(&mut self) -> Result<(), Trap> {
    let syscall = self.regs[17]; // a7
    match syscall {
        64 => self.sys_write(),
        93 => {
            let code = self.regs[10] as i32; // a0
            return Err(Trap::Exit(code));
        }
        214 => self.sys_brk(), // TODO
        222 => self.sys_mmap(), // TODO
        _ => {
            eprintln!(
                "[emu] unimplemented syscall {} (reading from a7 when `ecall` was called)",
                syscall
            );
            self.regs[10] = (-38i32) as u32; // -ENOSYS
        }
    }

    Ok(())
}

/// args: a0=fd, a1=ptr, a2=count
/// ret: a0=count or error
fn sys_write(&mut self) {
    let fd = self.regs[10]; // a0
    let ptr = self.regs[11]; // a1
    let count = self.regs[12]; // a2

    // copy bytes from memory
    let mut bytes = Vec::with_capacity(count as usize);
    for i in 0..count {
        bytes.push(self.bus.read_byte(ptr.wrapping_add(i)));
    }

    // stdout | stderr
    if fd == 1 || fd == 2 {
        use std::io::Write;
        let stream: &mut dyn Write = if fd == 1 {
            &mut std::io::stdout()
        } else {
            &mut std::io::stderr()
        };

        // always discard
        let _ = stream.write_all(&bytes);
        let _ = stream.flush(); // ensure everything reaches dest
        self.regs[10] = count; // normal ret 
    } else {
        self.regs[10] = (-9i32) as u32; // -EBADF
    }
}
```
Next on the list are `brk` and `mmap`, but they need their memory regions mapped out, specifically the heap. 
