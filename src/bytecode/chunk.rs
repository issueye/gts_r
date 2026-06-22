//! Compiled bytecode container: a flat instruction stream plus a constant
//! pool, a per-instruction source-position table, and (later) protected
//! regions for try/catch.
//!
//! Encoding is deliberately simple: `code` is a flat `Vec<u8>` where each
//! instruction is `<opcode byte> <operand bytes…>`. Operand widths are fixed
//! per opcode (see `Opcode`). Position tracking is line-anchored via
//! `line_offsets` to keep memory low; `position_at(ip)` does the lookup.

use crate::ast::Position;
use crate::object::Object;

use super::opcode::Opcode;

/// A try/catch protected region. Used from stage 6 onwards; declared here so
/// `Chunk` has a stable shape across stages.
#[derive(Debug, Clone)]
pub struct ProtectedRegion {
    /// Inclusive start ip of the try body.
    pub try_start: u32,
    /// Exclusive end ip of the try body (first byte after it).
    pub try_end: u32,
    /// Catch handler ip. Points past the try body.
    pub handler_ip: u32,
    /// Optional finally block ip.
    pub finally_ip: Option<u32>,
    /// Local slot that receives the caught value, if any.
    pub catch_binding_slot: Option<u8>,
}

/// A compiled function body / top-level program.
#[derive(Default)]
pub struct Chunk {
    /// Flat instruction stream.
    pub code: Vec<u8>,
    /// Constant pool. Referred to by `Const(u16)` etc.
    pub constants: Vec<Object>,
    /// Source position of the instruction that starts at each byte offset.
    /// Indexed by code offset; kept 1:1 with `code` for O(1) lookup. Memory is
    /// acceptable for stage 0; stage 8 may switch to run-length encoding.
    pub lines: Vec<Position>,
    /// Protected regions, sorted by `try_start`. Empty until stage 6.
    pub protected_regions: Vec<ProtectedRegion>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk::default()
    }

    /// Append a raw opcode byte (no operand) at the current offset, recording
    /// `pos` for error reporting.
    pub fn write_op(&mut self, op: Opcode, pos: Position) -> u32 {
        let offset = self.code.len() as u32;
        self.code.push(op as u8);
        self.lines.push(pos);
        offset
    }

    /// Append a single operand byte. Position inherited from the preceding
    /// opcode byte (callers write op first, then operands).
    pub fn write_byte(&mut self, b: u8, pos: Position) {
        self.code.push(b);
        self.lines.push(pos);
    }

    /// Append a u16 operand (big-endian, two bytes).
    pub fn write_u16(&mut self, v: u16, pos: Position) {
        self.write_byte((v >> 8) as u8, pos.clone());
        self.write_byte((v & 0xff) as u8, pos);
    }

    /// Append a u32 operand (big-endian, four bytes). Used by jump targets.
    pub fn write_u32(&mut self, v: u32, pos: Position) {
        self.write_byte(((v >> 24) & 0xff) as u8, pos.clone());
        self.write_byte(((v >> 16) & 0xff) as u8, pos.clone());
        self.write_byte(((v >> 8) & 0xff) as u8, pos.clone());
        self.write_byte((v & 0xff) as u8, pos);
    }

    /// Push a constant onto the pool and return its index. Deduplicates via
    /// `PartialEq` (numbers/strings/bools compare by value; reference types by
    /// shared `Rc` pointer).
    pub fn add_constant(&mut self, value: Object) -> u16 {
        // Linear scan is fine for stage 0; pools stay small. Revisit if stage
        // benchmarks show pressure.
        for (i, existing) in self.constants.iter().enumerate() {
            if existing == &value {
                return i as u16;
            }
        }
        let idx = self.constants.len() as u16;
        self.constants.push(value);
        idx
    }

    /// Read a u16 operand at the given byte offset (no bounds check beyond the
    /// slice indexing; callers ensure the offset is valid).
    pub fn read_u16(&self, ip: usize) -> u16 {
        let hi = self.code[ip] as u16;
        let lo = self.code[ip + 1] as u16;
        (hi << 8) | lo
    }

    /// Read a u32 operand at the given byte offset.
    pub fn read_u32(&self, ip: usize) -> u32 {
        let b0 = self.code[ip] as u32;
        let b1 = self.code[ip + 1] as u32;
        let b2 = self.code[ip + 2] as u32;
        let b3 = self.code[ip + 3] as u32;
        (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
    }

    /// Source position of the instruction starting at `ip`.
    pub fn position_at(&self, ip: usize) -> Position {
        self.lines
            .get(ip)
            .cloned()
            .unwrap_or_default()
    }

    /// Readable disassembly, primarily for debugging and stage-0 unit tests.
    pub fn disassemble(&self) -> String {
        let mut out = String::new();
        out.push_str("== constants ==\n");
        for (i, c) in self.constants.iter().enumerate() {
            out.push_str(&format!("  {:4} {:?}\n", i, c));
        }
        out.push_str("== code ==\n");
        let mut ip = 0;
        while ip < self.code.len() {
            let b = self.code[ip];
            let op = Opcode::from_byte(b);
            let pos = self.position_at(ip);
            match op {
                Some(op) => out.push_str(&format!(
                    "  {:4} {:<16} ; {}:{}:{}\n",
                    ip,
                    op.name(),
                    pos.file,
                    pos.line,
                    pos.col
                )),
                None => out.push_str(&format!("  {:4} <bad opcode 0x{:02x}>\n", ip, b)),
            }
            ip += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::num_obj;

    fn pos() -> Position {
        Position::new("t.gs", 1, 1, 0)
    }

    #[test]
    fn chunk_roundtrips_const_and_add() {
        let mut c = Chunk::new();
        let three = c.add_constant(num_obj(3.0));
        let four = c.add_constant(num_obj(4.0));
        // Same value pushed again dedups to the existing index.
        let three_again = c.add_constant(num_obj(3.0));
        assert_eq!(three_again, three);
        // CONST three ; CONST four ; ADD ; POP ; RETURN
        c.write_op(Opcode::Const, pos());
        c.write_u16(three, pos());
        c.write_op(Opcode::Const, pos());
        c.write_u16(four, pos());
        c.write_op(Opcode::Add, pos());
        c.write_op(Opcode::Pop, pos());
        c.write_op(Opcode::Return, pos());

        assert_eq!(c.constants.len(), 2); // 3.0 and 4.0 only
        assert_eq!(c.read_u16(1), 0);
        assert_eq!(c.read_u16(4), 1);
        assert_eq!(c.code[0], Opcode::Const as u8);
        assert_eq!(c.code[3], Opcode::Const as u8);
        assert_eq!(c.code[6], Opcode::Add as u8);
    }

    #[test]
    fn disassemble_is_readable() {
        let mut c = Chunk::new();
        let v = c.add_constant(num_obj(1.0));
        c.write_op(Opcode::Const, pos());
        c.write_u16(v, pos());
        c.write_op(Opcode::Return, pos());
        let s = c.disassemble();
        assert!(s.contains("CONST"));
        assert!(s.contains("RETURN"));
    }
}
