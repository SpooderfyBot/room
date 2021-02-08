#![allow(unused)]

pub type OpCode = usize;


pub const OP_PLAY: OpCode = 0;
pub const OP_PAUSE: OpCode = 1;
pub const OP_SEEK: OpCode = 2;
pub const OP_NEXT: OpCode = 3;
pub const OP_PREV: OpCode = 4;
pub const OP_MESSAGE: OpCode = 5;
pub const OP_TIME_CHECK: OpCode = 6;
pub const OP_ADD_TRACK: OpCode = 7;
pub const OP_REMOVE_TRACK: OpCode = 8;
pub const OP_SYNC_TRACKS: OpCode = 9;
pub const OP_SET_BULK_TRACKS: OpCode = 10;
