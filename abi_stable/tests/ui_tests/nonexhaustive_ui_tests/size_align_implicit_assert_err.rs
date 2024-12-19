#![allow(warnings, unused_unsafe)]

use abi_stable::StableAbi;

#[repr(C, u8)]
#[derive(StableAbi)]
#[sabi(kind(WithNonExhaustive(size = 1, align = 1,)))]
#[sabi(with_constructor)]
pub enum TooLarge {
    Foo,
    Bar,
    Baz(u8),
}

#[repr(C, u8)]
#[derive(StableAbi)]
#[sabi(kind(WithNonExhaustive(size = 32, align = 1,)))]
#[sabi(with_constructor)]
pub enum Unaligned {
    Foo,
    Bar,
    Baz(u64),
}

#[repr(C, u8)]
#[derive(StableAbi)]
#[sabi(kind(WithNonExhaustive(size = 1, align = 1,)))]
#[sabi(with_constructor)]
pub enum UnalignedAndTooLarge {
    Foo,
    Bar,
    Baz(u64),
}

fn main() {}
