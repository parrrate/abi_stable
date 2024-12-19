//! This is an `implementation crate`,
//! It exports the root module(a struct of function pointers) required by the
//! `example_0_interface`(the `interface crate`).

use std::{
    borrow::Cow,
    collections::HashSet,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
};

use example_0_interface::{
    DeserializerMod, DeserializerMod_Ref, RemoveWords, TOCommand, TOCommandBox, TOReturnValue,
    TOReturnValueArc, TOState, TOStateBox, TextOpsMod, TextOpsMod_Ref,
};

use abi_stable::{
    erased_types::SerializeType,
    export_root_module,
    external_types::RawValueBox,
    prefix_type::{PrefixTypeTrait, WithMetadata},
    sabi_extern_fn,
    std_types::{RArc, RBox, RBoxError, RCow, RErr, ROk, RResult, RStr, RString, RVec},
    traits::IntoReprC,
    DynTrait,
};
use core_extensions::{SelfOps, StringExt};

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

///////////////////////////////////////////////////////////////////////////////////

/// Exports the root module of this library.
///
/// This code isn't run until the layout of the type it returns is checked.
#[export_root_module]
// #[unsafe_no_layout_constant]
fn instantiate_root_module() -> TextOpsMod_Ref {
    TextOpsMod {
        new,
        deserializers: {
            // Another way to instantiate a module.
            const MOD_: DeserializerMod = DeserializerMod {
                something: PhantomData,
                deserialize_state,
                deserialize_command,
                deserialize_command_borrowing,
                deserialize_return_value,
            };

            const S: WithMetadata<DeserializerMod> = WithMetadata::new(MOD_);

            DeserializerMod_Ref(S.static_as_prefix())
        },
        reverse_lines,
        remove_words,
        get_processed_bytes,
        set_initial_processed_bytes,
        run_command,
    }
    .leak_into_prefix()
}

///////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TextOperationState {
    processed_bytes: u64,
}

/// Defines how the type is serialized in DynTrait<_>.
impl<'a> SerializeType<'a> for TextOperationState {
    type Interface = TOState;

    fn serialize_impl(&'a self) -> Result<RawValueBox, RBoxError> {
        serialize_json(self)
    }
}

//////////////////////////////////////////////////////////////////////////////////////

/// An enum used to send commands to this library dynamically.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Command<'a> {
    ReverseLines(RString),
    RemoveWords {
        string: RString,
        words: RVec<RString>,
        #[serde(skip)]
        _marker: PhantomData<&'a mut RString>,
    },
    GetProcessedBytes,
    Batch(RVec<Command<'a>>),
}

impl<'a> Iterator for Command<'a> {
    type Item = &'a mut RString;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// Defines how the type is serialized in DynTrait<_>.
impl<'a> SerializeType<'a> for Command<'_> {
    type Interface = TOCommand;
    fn serialize_impl(&'a self) -> Result<RawValueBox, RBoxError> {
        serialize_json(self)
    }
}

//////////////////////////////////////////////////////////////////////////////////////

/// The return type of `fn run_command`,
/// where the returned variant corresponds to the `Command` that was passed in.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ReturnValue {
    ReverseLines(RString),
    RemoveWords(RString),
    GetProcessedBytes(u64),
    Batch(RVec<ReturnValue>),
}

/// Defines how the type is serialized in DynTrait<_>.
impl<'a> SerializeType<'a> for ReturnValue {
    type Interface = TOReturnValue;
    fn serialize_impl(&'a self) -> Result<RawValueBox, RBoxError> {
        serialize_json(self)
    }
}

//////////////////////////////////////////////////////////////////////////////////////

fn deserialize_json<'a, T>(s: RStr<'a>) -> RResult<T, RBoxError>
where
    T: serde::Deserialize<'a>,
{
    match serde_json::from_str::<T>(s.into()) {
        Ok(x) => ROk(x),
        Err(e) => RErr(RBoxError::new(e)),
    }
}

fn serialize_json<T>(value: &T) -> Result<RawValueBox, RBoxError>
where
    T: serde::Serialize,
{
    match serde_json::to_string::<T>(value) {
        Ok(v) => unsafe { Ok(RawValueBox::from_rstring_unchecked(v.into_c())) },
        Err(e) => Err(RBoxError::new(e)),
    }
}

//////////////////////////////////////////////////////////////////////////////////////

/// Defines how a TOStateBox is deserialized from json.
#[sabi_extern_fn]
pub fn deserialize_state(s: RStr<'_>) -> RResult<TOStateBox, RBoxError> {
    deserialize_json::<TextOperationState>(s).map(DynTrait::from_value)
}

/// Defines how a TOCommandBox is deserialized from json.
#[sabi_extern_fn]
pub fn deserialize_command(s: RStr<'_>) -> RResult<TOCommandBox<'static>, RBoxError> {
    deserialize_json::<Command>(s)
        .map(RBox::new)
        .map(DynTrait::from_ptr)
}

/// Defines how a TOCommandBox is deserialized from json.
#[sabi_extern_fn]
pub fn deserialize_command_borrowing(s: RStr<'_>) -> RResult<TOCommandBox<'_>, RBoxError> {
    deserialize_json::<Command>(s)
        .map(RBox::new)
        .map(DynTrait::from_borrowing_ptr)
}

/// Defines how a TOReturnValueArc is deserialized from json.
#[sabi_extern_fn]
pub fn deserialize_return_value(s: RStr<'_>) -> RResult<TOReturnValueArc, RBoxError> {
    deserialize_json::<ReturnValue>(s)
        .map(RArc::new)
        .map(DynTrait::from_ptr)
}

//////////////////////////////////////////////////////////////////////////////////////

static INITIAL_PROCESSED_BYTES: AtomicU64 = AtomicU64::new(0);

/// Constructs a TextOperationState and erases it by wrapping it into a
/// `DynTrait<Box<()>,TOState>`.
#[sabi_extern_fn]
pub fn new() -> TOStateBox {
    let this = TextOperationState {
        processed_bytes: INITIAL_PROCESSED_BYTES.load(Ordering::SeqCst),
    };
    DynTrait::from_value(this)
}

/// Reverses order of the lines in `text`.
#[sabi_extern_fn]
pub fn reverse_lines(this: &mut TOStateBox, text: RStr<'_>) -> RString {
    let this = this.downcast_as_mut::<TextOperationState>().unwrap();

    this.processed_bytes += text.len() as u64;

    let mut lines = text.lines().collect::<Vec<&str>>();
    lines.reverse();
    let mut buffer = RString::with_capacity(text.len());
    for line in lines {
        buffer.push_str(line);
        buffer.push('\n');
    }
    buffer
}

/// Removes the words in `param.words` from `param.string`,
/// as well as the whitespace that comes after it.
#[sabi_extern_fn]
// How is a `&mut ()` not ffi-safe?????
#[allow(improper_ctypes_definitions)]
pub fn remove_words(this: &mut TOStateBox, param: RemoveWords<'_, '_>) -> RString {
    let this = this.downcast_as_mut::<TextOperationState>().unwrap();

    this.processed_bytes += param.string.len() as u64;

    let set = param
        .words
        .map(RCow::into)
        .collect::<HashSet<Cow<'_, str>>>();
    let mut buffer = String::with_capacity(10);

    let haystack = &*param.string;
    let mut prev_was_deleted = false;
    for kv in haystack.split_while(|c| c.is_alphabetic()) {
        let s = kv.str;
        let cs = Cow::from(s);
        let is_a_word = kv.key;
        let is_deleted = (!is_a_word && prev_was_deleted) || (is_a_word && set.contains(&cs));
        if !is_deleted {
            buffer.push_str(s);
        }
        prev_was_deleted = is_deleted;
    }

    buffer.into()
}

/// Returns the amount of text (in bytes)
/// that was processed in functions taking `&mut TOStateBox`.
#[sabi_extern_fn]
pub fn get_processed_bytes(this: &TOStateBox) -> u64 {
    let this = this.downcast_as::<TextOperationState>().unwrap();
    this.processed_bytes
}

#[sabi_extern_fn]
pub fn set_initial_processed_bytes(n: u64) {
    INITIAL_PROCESSED_BYTES.store(n, Ordering::SeqCst);
}

fn run_command_inner(this: &mut TOStateBox, command: Command) -> ReturnValue {
    match command {
        Command::ReverseLines(s) => {
            reverse_lines(this, s.as_rstr()).piped(ReturnValue::ReverseLines)
        }
        Command::RemoveWords {
            string,
            words,
            _marker: _,
        } => {
            let iter = &mut words.iter().map(|s| RCow::from(s.as_rstr()));

            remove_words(
                this,
                RemoveWords {
                    string: string.as_rstr(),
                    words: DynTrait::from_borrowing_ptr(iter),
                },
            )
            .piped(ReturnValue::RemoveWords)
        }
        Command::GetProcessedBytes => {
            get_processed_bytes(this).piped(ReturnValue::GetProcessedBytes)
        }
        Command::Batch(list) => list
            .into_iter()
            .map(|cmd| run_command_inner(this, cmd))
            .collect::<RVec<ReturnValue>>()
            .piped(ReturnValue::Batch),
    }
}

/// An interpreter for text operation commands
// How is a `*mut ()` not ffi-safe?????
#[allow(improper_ctypes_definitions)]
#[sabi_extern_fn]
pub fn run_command(this: &mut TOStateBox, command: TOCommandBox<'static>) -> TOReturnValueArc {
    let command = command
        .downcast_into::<Command<'static>>()
        .unwrap()
        .piped(RBox::into_inner);

    run_command_inner(this, command)
        .piped(RArc::new)
        .piped(DynTrait::from_ptr)
}
