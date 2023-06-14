use ffxivclientstructs::{MemberFunctionSignature, StaticAddressSignature, VTableSignature};

pub unsafe fn resolve_vtable(input: &VTableSignature) -> Option<*const usize> {
    // TODO
    None
}

pub unsafe fn resolve_static_address(input: &StaticAddressSignature) -> Option<*const usize> {
    // TODO
    None
}

pub unsafe fn resolve_member_function(input: &MemberFunctionSignature) -> Option<*const usize> {
    // TODO
    None
}
