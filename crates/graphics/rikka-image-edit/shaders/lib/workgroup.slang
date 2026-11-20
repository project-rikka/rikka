typealias SharedPointer<T> = Ptr<T, Access.ReadWrite, AddressSpace.GroupShared>;

[ForceInline]
T shared_load<T>(SharedPointer<T> element) {
    return loadCoherent<4, MemoryScope.Workgroup>(element);
}

[ForceInline]
void shared_store<T>(SharedPointer<T> element, T value) {
    storeCoherent<4, MemoryScope.Workgroup>(element, value);
}
