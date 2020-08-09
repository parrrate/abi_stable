use super::*;

use crate::{
    prefix_type::{PrefixRef, PrefixRefTrait},
    sabi_types::Constructor,
};

/// Used to check the layout of modules returned by module-loading functions
/// exported by dynamic libraries.
#[repr(C)]
#[derive(StableAbi)]
pub struct LibHeader {
    header:AbiHeader,
    root_mod_consts:ErasedRootModuleConsts,
    init_globals_with:InitGlobalsWith,
    module:LateStaticRef<PrefixRef<ErasedPrefix>>,
    constructor:Constructor<PrefixRef<ErasedPrefix>>,
}

impl LibHeader {
    /// Constructs a LibHeader from the root module loader.
    pub const unsafe fn from_constructor<M>(
        constructor:Constructor<PrefixRef<ErasedPrefix>>,
        root_mod_consts:RootModuleConsts<M>,
    )->Self
    {
        Self {
            header:AbiHeader::VALUE,
            root_mod_consts:root_mod_consts.erased(),
            init_globals_with: INIT_GLOBALS_WITH,
            module:LateStaticRef::new(),
            constructor:constructor,
        }
    }

    /// Constructs a LibHeader from the module.
    pub fn from_module<T>(value:T)->Self
    where
        T: RootModule,
    {
        Self {
            header:AbiHeader::VALUE,
            root_mod_consts: T::CONSTANTS.erased(),
            init_globals_with: INIT_GLOBALS_WITH,
            module: {
                let erased = unsafe{ value.to_prefix_ref().cast::<ErasedPrefix>() }; 
                LateStaticRef::from_prefixref(PrefixRefTrait::PREFIX_FIELDS, erased)
            },
            constructor:GetAbortingConstructor::ABORTING_CONSTRUCTOR,
        }
    }

    /// All the important constants of a `RootModule` for some erased type.
    pub fn root_mod_consts(&self)->&ErasedRootModuleConsts{
        &self.root_mod_consts
    }

    /// The version string of the library the module is being loaded from.
    pub fn version_strings(&self)->VersionStrings{
        self.root_mod_consts.version_strings()
    }

    /// Gets the layout of the root module.
    ///
    /// This returns a None if the root module layout is not included
    /// because the `#[unsafe_no_layout_constant]` 
    /// helper attribute was used on the function exporting the root module.
    pub fn layout(&self)->Option<&'static TypeLayout>{
        self.root_mod_consts.layout().into_option()
    }

    pub(super) fn initialize_library_globals(&self,globals:&'static Globals){
        (self.init_globals_with.0)(globals);
    }


    fn check_version<M>(&self)->Result<(),LibraryError>
    where
        M:RootModule
    {
        let expected_version = M::VERSION_STRINGS
            .piped(VersionNumber::new)?;

        let actual_version = self.version_strings().piped(VersionNumber::new)?;

        if expected_version.major != actual_version.major || 
            (expected_version.major==0) && expected_version.minor > actual_version.minor
        {
            return Err(LibraryError::IncompatibleVersionNumber {
                library_name: M::NAME,
                expected_version,
                actual_version,
            });
        }
        Ok(())
    }


    /**
Checks that the library is compatible,returning the root module on success.

It checks that these are compatible:

- The version number of the library

- The layout of the root module.

# Warning

If this function is called within a dynamic library,
it must be called at or after the function that exports its root module is called.

**DO NOT** call this in the static initializer of a dynamic library,
since this library relies on setting up its global state before
calling the root module loader.

# Errors

This will return these errors:

- LibraryError::ParseVersionError:
If the version strings in the library can't be parsed as version numbers,
this can only happen if the version strings are manually constructed.

- LibraryError::IncompatibleVersionNumber:
If the version number of the library is incompatible.

- LibraryError::AbiInstability:
If the layout of the root module is not the expected one.



    */
    pub fn init_root_module<M>(&self)-> Result<M, LibraryError>
    where
        M: RootModule
    {
        self.check_version::<M>()?;
        self.check_layout::<M>()
    }



    /**
Checks that the version number of the library is compatible,
returning the root module on success.

This function transmutes the root module type,
without checking that the layout is compatible first.

# Warning

If this function is called within a dynamic library,
it must be called at or after the function that exports its root module is called.

**DO NOT** call this in the static initializer of a dynamic library,
since this library relies on setting up its global state before
calling the root module loader.

# Safety

The caller must ensure that `M` has the expected layout.

# Errors

This will return these errors:

- LibraryError::ParseVersionError:
If the version strings in the library can't be parsed as version numbers,
this can only happen if the version strings are manually constructed.

- LibraryError::IncompatibleVersionNumber:
If the version number of the library is incompatible.

    */
    pub unsafe fn init_root_module_with_unchecked_layout<M>(
        &self
    )-> Result<M, LibraryError>
    where
        M: RootModule
    {
        self.check_version::<M>()?;
        Ok(self.unchecked_layout())
    }


    /// Gets the root module,first 
    /// checking that the layout of the `M` from the dynamic library is 
    /// compatible with the expected layout.
    pub fn check_layout<M>(&self) -> Result<M, LibraryError>
    where
        M: RootModule,
    {
        if let IsLayoutChecked::Yes(root_mod_layout)=self.root_mod_consts.layout(){
            // Using this instead of
            // crate::abi_stability::abi_checking::check_layout_compatibility
            // so that if this is called in a dynamic-library that loads 
            // another dynamic-library,
            // it uses the layout checker of the executable,
            // ensuring a globally unique view of the layout of types.
            //
            // This might also reduce the code in the library,
            // because it doesn't have to compile the layout checker for every library.
            (globals::initialized_globals().layout_checking)
                (<M>::LAYOUT, root_mod_layout)
                .into_result()
                .map_err(|e|{
                    // Fixes the bug where printing the error causes a segfault because it 
                    // contains static references and function pointers into the unloaded library.
                    let formatted = e.to_formatted_error();
                    LibraryError::AbiInstability(formatted)
                })?;
        }
        
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        
        let prefix_ref = unsafe{
            self.module.init(|| (self.constructor.0)() )
                .cast::<M::PrefixFields>()
        };
        Ok(M::from_prefix_ref(prefix_ref))
    }


/**
Gets the root module without checking that the layout of `M` is the expected one.
This is effectively a transmute.

This is useful if a user keeps a cache of which dynamic libraries 
have been checked for layout compatibility.

# Safety

The caller must ensure that `M` has the expected layout.

*/
    pub unsafe fn unchecked_layout<M>(&self)->M
    where
        M: PrefixRefTrait,
    {
        self.module.init(|| (self.constructor.0)() )
            .cast::<M::PrefixFields>()
            .piped(M::from_prefix_ref)
    }
}

//////////////////////////////////////////////////////////////////////


struct GetAbortingConstructor<T>(T);

impl<T> GetAbortingConstructor<T>{
    const ABORTING_CONSTRUCTOR:Constructor<T>=
        Constructor(Self::aborting_constructor);

    extern "C" fn aborting_constructor()->T{
        extern_fn_panic_handling!{
            panic!(
                "BUG:\n\
                 This function \
                 (abi_stable::library::lib_header::GetAbortingConstructor::aborting_constructor) \
                 must only be used \
                 as a dummy functions when initializing `LibHeader` \
                 within `LibHeader::from_module`."
            );
        }
    }
}


//////////////////////////////////////////////////////////////////////

#[repr(C)]
#[derive(StableAbi,Copy,Clone)]
struct InitGlobalsWith(pub extern "C" fn(&'static Globals));

const INIT_GLOBALS_WITH:InitGlobalsWith=
    InitGlobalsWith(crate::globals::initialize_globals_with);


//////////////////////////////////////////////////////////////////////

/**
Represents the abi_stable version used by a compiled dynamic library,
which if incompatible would produce a `LibraryError::InvalidAbiHeader`

*/
#[repr(C)]
#[derive(Debug,StableAbi,Copy,Clone)]
pub struct AbiHeader{
    /// A magic string used to check that this is actually abi_stable.
    pub magic_string:[u8;32],
    /// The major abi version of abi_stable
    pub abi_major:u32,
    /// The minor abi version of abi_stable
    pub abi_minor:u32,
    _priv:(),
}


impl AbiHeader{
    /// The value of the AbiHeader stored in dynamic libraries that use this 
    /// version of abi_stable
    pub const VALUE:AbiHeader={
        mod value{
            use super::*;
            abi_stable_derive::construct_abi_header!{}
        }
        value::ABI_HEADER
    };
}



impl AbiHeader{
    /// Checks whether this AbiHeader is compatible with `other`.
    pub fn is_compatible(&self,other:&Self)->bool{
        self.magic_string == other.magic_string&&
        self.abi_major    == other.abi_major   &&
        ( self.abi_major!=0 || self.abi_minor==other.abi_minor )
    }

    /// Checks whether the abi_stable version of this AbiHeader is 
    /// compatible with the one from where this function is called.
    pub fn is_valid(&self)->bool{
        self.is_compatible(&AbiHeader::VALUE)
    }

    /**
Gets the LibHeader of a library.

# Errors

This will return these errors:

- LibraryError::InvalidAbiHeader:
If the abi_stable used by the library is not compatible.

- LibraryError::InvalidCAbi:
If the C abi used by the library is not compatible.

    */
    pub fn upgrade(&self)->Result< &LibHeader , LibraryError>{
        if !self.is_valid() {
            return Err(LibraryError::InvalidAbiHeader(*self))
        }

        let lib_header=unsafe{
            transmute_reference::<AbiHeader,LibHeader>(self)
        };

        let c_abi_testing_fns=lib_header.root_mod_consts().c_abi_testing_fns();
        crate::library::c_abi_testing::run_tests(c_abi_testing_fns)?;

        let globals=globals::initialized_globals();

        lib_header.initialize_library_globals(globals);

        Ok(lib_header)
    }
}
