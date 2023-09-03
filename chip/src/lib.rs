use core::fmt;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

/// CHIP-8 虚拟机内存的前 512 字节通常是由解释器自身占用的，最后 256 字节被保留用于显示刷新
/// 因此这里程序入口地址为 512
pub const ENTRY_ADDR: u16 = 512;

/// CHIP-8 虚拟机可以显示 64 x 32 的单色像素内容
pub const DISP_WIDTH: usize = 64;
/// CHIP-8 虚拟机可以显示 64 x 32 的单色像素内容
pub const DISP_HEIGHT: usize = 32;

/// CHIP-8 虚拟机有 4KiB 的内存空间
const MEM_SIZE: usize = 4096;
/// CHIP-8 虚拟机的栈大小是 16 x 16-bit
const STACK_SIZE: usize = 16;
/// CHIP-8 虚拟机的有 16 个 8-bit 寄存器
const REG_NUM: usize = 16;

/// 字体 0 ~ F, 共 16 个字符
const CHARS_SIZE: usize = 5 * 16;

const CHARS: [u8; CHARS_SIZE] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

#[derive(Debug)]
pub enum Exception {
    OutOfMemory(u16),
    StackOverflow(u8),
    StackUnderflow(u8),
    IllegalOpcode(u16),
    IllegalAddress(u16),
    Halt(i32),
}

pub struct Chip {
    mem: [u8; MEM_SIZE],
    v: [u8; REG_NUM], // 寄存器组
    i: u16,           // 索引寄存器
    pc: u16,          // 程序计数器
    stack: [u16; STACK_SIZE],
    sp: u8,                               // 栈指针
    dt: u8,                               // 延迟定时器
    st: u8,                               // 声音定时器
    keypad: [bool; 16],                   // 键盘
    fb: [bool; DISP_WIDTH * DISP_HEIGHT], // 显示帧缓冲，这里用一个布尔值来表示一个像素，方便后续操作
    rng: SmallRng,                        // 随机数生成器
}

impl fmt::Display for Chip {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "PC: {:04X} SP: {:02X} I: {:04X} KEY: {}",
            self.pc,
            self.sp,
            self.i,
            self.keypad
                .iter()
                .enumerate()
                .find(|(i, &x)| x)
                .map_or("None".to_string(), |(i, _)| i.to_string())
        )?;
        writeln!(
            f,
            "DT: {:02X} ST: {:02X} Stack: {:?}",
            self.dt,
            self.st,
            &self.stack[..self.sp as usize]
        )?;
        writeln!(
            f,
            "V0: {:02X} V1: {:02X} V2: {:02X} V3: {:02X}",
            self.v[0], self.v[1], self.v[2], self.v[3]
        )?;
        writeln!(
            f,
            "V4: {:02X} V5: {:02X} V6: {:02X} V7: {:02X}",
            self.v[4], self.v[5], self.v[6], self.v[7]
        )?;
        writeln!(
            f,
            "V8: {:02X} V9: {:02X} Va: {:02X} Vb: {:02X}",
            self.v[8], self.v[9], self.v[10], self.v[11]
        )?;
        write!(
            f,
            "Vc: {:02X} Vd: {:02X} Ve: {:02X} Vf: {:02X}",
            self.v[12], self.v[13], self.v[14], self.v[15]
        )
    }
}

impl Chip {
    pub fn new(seed: u64) -> Self {
        let mut mem = [0; MEM_SIZE];
        mem[..CHARS_SIZE].copy_from_slice(&CHARS);
        Self {
            mem,
            v: [0; REG_NUM],
            i: 0,
            pc: ENTRY_ADDR,
            stack: [0; STACK_SIZE],
            sp: 0,
            dt: 0,
            st: 0,
            keypad: [false; 16],
            fb: [false; DISP_WIDTH * DISP_HEIGHT],
            rng: SmallRng::seed_from_u64(seed),
        }
    }

    /// 模拟系统时钟滴答，自动取指执行
    pub fn tick(&mut self) -> Result<(), Exception> {
        if self.dt > 0 {
            self.dt -= 1;
        }
        if self.st > 0 {
            self.st -= 1;
        }

        if self.pc >= MEM_SIZE as u16 {
            return Err(Exception::OutOfMemory(self.pc));
        }
        let op = self.fetch();
        self.pc += 2;
        self.execute(op)?;

        Ok(())
    }

    /// 设置虚拟机键盘状态
    pub fn set_keypad(&mut self, key: u8, pressed: bool) {
        if key < 16 {
            // println!(
            //     "Key [{}] {}",
            //     key,
            //     if pressed { "pressed" } else { "released." }
            // );
            self.keypad[key as usize] = pressed;
            // println!("Keypad: {:?}", self.keypad);
        }
    }

    /// 装载程序
    pub fn load_rom(&mut self, offset: u16, bin: &[u8]) -> Result<(), Exception> {
        if bin.len() > (MEM_SIZE - offset as usize) {
            return Err(Exception::OutOfMemory(bin.len() as u16));
        }
        self.mem[offset as usize..offset as usize + bin.len()].copy_from_slice(&bin);
        Ok(())
    }

    /// 获取显示帧缓冲
    pub fn framebuffer(&self) -> &[bool] {
        &self.fb
    }

    /// 获取音调输出
    pub fn tone(&self) -> bool {
        self.st != 0
    }

    /// 虚拟机复位
    pub fn reset(&mut self, seed: u64) {
        self.pc = ENTRY_ADDR;
        self.sp = 0;
        self.i = 0;
        self.dt = 0;
        self.st = 0;
        self.keypad.fill(false);
        self.fb.fill(false);
        self.v.fill(0);
        self.mem.fill(0);
        self.stack.fill(0);
        self.rng = SmallRng::seed_from_u64(seed);
    }

    // 取指令
    fn fetch(&mut self) -> u16 {
        // 使用的是大端模式 (数据高字节在低地址上)
        (self.mem[self.pc as usize] as u16) << 8 | (self.mem[self.pc as usize + 1] as u16)
    }

    // 执行指令
    fn execute(&mut self, opcode: u16) -> Result<(), Exception> {
        let d = (opcode & 0xF000) >> 12;
        let x = ((opcode & 0x0F00) >> 8) as u8;
        let y = ((opcode & 0x00F0) >> 4) as u8;
        let n = (opcode & 0x000F) as u8;
        let nn = (opcode & 0x00FF) as u8;
        let nnn = opcode & 0x0FFF;

        let vx = self.v[x as usize];
        let vy = self.v[y as usize];

        // println!("op:{opcode:04X}, d:{d:01X}, x:{x:01X}, y:{y:01X}, n:{n}, nn:{nn}, nnn:{nnn:03X}");

        match d {
            0 => match nn {
                // NOP
                0 => (),
                0xE0 => self.disp_clr(),
                0xEE => self.ret()?,
                _ => return Err(Exception::IllegalOpcode(opcode)),
            },
            1 => self.jump(nnn)?,
            2 => self.call(nnn)?,
            3 => self.skip_if_eq(vx, nn),
            4 => self.skip_if_ne(vx, nn),
            5 => self.skip_if_eq(vx, vy),
            6 => self.load_reg(x, nn),
            7 => self.load_reg(x, vx.wrapping_add(nn)),
            8 => match n {
                0 => self.load_reg(x, vy),
                1 => self.load_reg(x, vx | vy),
                2 => self.load_reg(x, vx & vy),
                3 => self.load_reg(x, vx ^ vy),
                4 => {
                    let (val, carry) = vx.overflowing_add(vy);
                    self.load_reg(x, val);
                    self.load_reg(0xFu8, if carry { 1 } else { 0 });
                }
                5 => {
                    let (val, borrow) = vx.overflowing_sub(vy);
                    self.load_reg(x, val);
                    self.load_reg(0xFu8, if borrow { 0 } else { 1 });
                }
                6 => {
                    self.load_reg(0xFu8, vx & 0x01);
                    self.load_reg(x, vx >> 1);
                }
                7 => {
                    let (val, borrow) = vy.overflowing_sub(vx);
                    self.load_reg(x, val);
                    self.load_reg(0xFu8, if borrow { 0 } else { 1 });
                }
                0xE => {
                    self.load_reg(0xFu8, if vx & 0x80 == 0 { 0 } else { 1 });
                    self.load_reg(x, vx << 1);
                }
                _ => return Err(Exception::IllegalOpcode(opcode)),
            },
            9 => self.skip_if_ne(vx, vy),
            0xA => self.load_i(nnn),
            0xB => self.jump(self.v[0] as u16 + nnn)?,
            0xC => {
                let r = self.rng.gen::<u8>() % nn;
                self.load_reg(x, r);
            }
            0xD => self.draw_sprite(x, y, n),
            0xE => match nn {
                0x9E => {
                    // 如果 Vx 对应的按键按下，则跳过下一条指令
                    let keypad = self.keypad.iter().enumerate().find(|(_, &k)| k);
                    if let Some((i, _)) = keypad {
                        if vx == i as u8 {
                            self.pc += 2;
                        }
                    }
                }
                0xA1 => {
                    // 如果 Vx 对应的按键没有按下，则跳过下一条指令
                    let keypad = self.keypad.iter().enumerate().find(|(_, &k)| k);
                    if let Some((i, _)) = keypad {
                        if vx != i as u8 {
                            self.pc += 2;
                        }
                    }
                }
                _ => return Err(Exception::IllegalOpcode(opcode)),
            },
            0xF => match nn {
                0x07 => self.load_reg(x, self.dt),
                0x0A => self.wait_for_key(x),
                0x15 => {
                    self.dt = vx;
                }
                0x18 => {
                    self.st = vx;
                }
                0x1E => self.load_i(self.i + vx as u16),
                0x29 => self.load_i(5 * vx as u16),
                0x33 => self.store_reg_bcd(x),
                0x55 => self.store_regs(x)?,
                0x65 => self.load_regs(x)?,
                _ => return Err(Exception::IllegalOpcode(opcode)),
            },
            _ => return Err(Exception::IllegalOpcode(opcode)),
        }
        Ok(())
    }

    fn disp_clr(&mut self) {
        self.fb.fill(false);
    }

    fn ret(&mut self) -> Result<(), Exception> {
        if self.sp == 0 {
            return Err(Exception::StackUnderflow(self.sp));
        }
        // 出栈
        self.sp -= 1;
        self.pc = self.stack[self.sp as usize];

        Ok(())
    }

    fn jump(&mut self, addr: u16) -> Result<(), Exception> {
        if addr > 0xFFF {
            return Err(Exception::IllegalAddress(addr));
        }
        self.pc = addr;

        Ok(())
    }

    fn call(&mut self, addr: u16) -> Result<(), Exception> {
        if self.sp >= STACK_SIZE as u8 {
            return Err(Exception::StackOverflow(self.sp));
        }
        // 压栈
        self.stack[self.sp as usize] = self.pc;
        self.sp += 1;

        if addr > 0xFFF {
            return Err(Exception::IllegalAddress(addr));
        }
        // 跳转
        self.pc = addr;

        Ok(())
    }

    fn skip_if_eq(&mut self, a: u8, b: u8) {
        if a == b {
            self.pc += 2;
        }
    }

    fn skip_if_ne(&mut self, a: u8, b: u8) {
        if a != b {
            self.pc += 2;
        }
    }

    fn load_reg(&mut self, x: u8, val: u8) {
        self.v[x as usize] = val;
    }

    fn load_i(&mut self, val: u16) {
        self.i = val;
    }

    fn draw_sprite(&mut self, x: u8, y: u8, n: u8) {
        let x = self.v[x as usize] as usize;
        let y = self.v[y as usize] as usize;
        let n = n as usize;
        let mut flipped = false;
        for i in 0..n {
            let sprite = self.mem[self.i as usize + i];
            for j in 0..8 {
                // 判断是否反转像素颜色
                if sprite & (0x80 >> j) != 0 {
                    let idx = (x + j) % DISP_WIDTH + ((y + i) % DISP_HEIGHT) * DISP_WIDTH;
                    // 如果之前的像素是白色，则反转就是黑色，设置 flip 标志
                    flipped |= self.fb[idx];
                    // 反转当前像素
                    self.fb[idx] ^= true;
                }
            }
        }
        self.v[0xF] = if flipped { 1 } else { 0 };
    }

    fn wait_for_key(&mut self, x: u8) {
        // println!("Wait for key: {}", self.v[x as usize]);
        let keypad = self
            .keypad
            .iter()
            .enumerate()
            .find(|(i, &k)| self.v[x as usize] == *i as u8 && k);

        if keypad.is_some() {
            self.pc += 2;
        }
    }

    fn store_reg_bcd(&mut self, x: u8) {
        let mut bcd = [0u8; 3];
        let num = self.v[x as usize];
        let (div, num) = (num / 100, num % 100);
        bcd[0] = div;
        let (div, num) = (num / 10, num % 10);
        bcd[1] = div;
        bcd[2] = num;
        self.mem[self.i as usize..self.i as usize + 3].copy_from_slice(&bcd);
    }

    fn store_regs(&mut self, x: u8) -> Result<(), Exception> {
        let mut offset = self.i as usize;
        for i in 0..x as usize {
            if offset < MEM_SIZE {
                self.mem[offset] = self.v[i];
                offset += 1;
            } else {
                return Err(Exception::IllegalAddress(offset as u16));
            }
        }

        Ok(())
    }

    fn load_regs(&mut self, x: u8) -> Result<(), Exception> {
        let mut offset = self.i as usize;
        for i in 0..x as usize {
            if offset < MEM_SIZE {
                self.v[i] = self.mem[offset];
                offset += 1;
            } else {
                return Err(Exception::IllegalAddress(offset as u16));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::random;

    use super::*;

    #[test]
    fn test_load_rom() {
        let mut cpu = Chip::new(0);
        let offset = random::<usize>() % (MEM_SIZE - 8);
        cpu.load_rom(offset as u16, &[1u8, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        assert_eq!(cpu.mem[offset..offset + 8], [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_reg_op() {
        let mut cpu = Chip::new(0);
        cpu.load_rom(
            ENTRY_ADDR,
            &[
                0x60, 0x0F, // V0 = 15
                0x81, 0x00, // V1 = V0 => V1 = 15
                0x70, 0x0A, // V0 += 10 => V0 = 25
                0x80, 0x11, // V0 |= V1 => V0 = 31
            ],
        )
        .unwrap();

        cpu.tick().unwrap();
        assert_eq!(cpu.v[0], 15);
        cpu.tick().unwrap();
        assert_eq!(cpu.v[1], 15);
        cpu.tick().unwrap();
        assert_eq!(cpu.v[0], 25);
        cpu.tick().unwrap();
        assert_eq!(cpu.v[0], 31);
    }
}
