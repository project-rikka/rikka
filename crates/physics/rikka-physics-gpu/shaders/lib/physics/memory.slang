interface INonWritable {
    static This load_from<let ALIGN : int>(This* address);
}

T load_non_writable<let ALIGN : int, T : INonWritable>(T* address) {
    return T.load_from<ALIGN>(address);
}

T load_non_writable_plain<let ALIGN : int, T>(T* address) {
#pragma warning(push)
#pragma warning(disable : 29116)
    return spirv_asm {
        %view = OpTypeStruct $$T;
        OpMemberDecorate %view 0 Offset 0;
        OpMemberDecorate %view 0 NonWritable;
        %view_pointer = OpTypePointer PhysicalStorageBuffer %view;
        %element_pointer = OpTypePointer PhysicalStorageBuffer $$T;
        %base = OpBitcast %view_pointer $address;
        %element = OpAccessChain %element_pointer %base $(0);
        result:$$T = OpLoad %element Aligned !ALIGN
    };
#pragma warning(pop)
}

__generic<T : IArithmetic>
extension T : INonWritable {
    static T load_from<let ALIGN : int>(T* address) {
        return load_non_writable_plain<ALIGN>(address);
    }
}

struct NonWritableBuffer<T : INonWritable, let ALIGN : int> : INonWritable {
    T* address;

    __subscript(uint i)->T {
        get {
            return load_non_writable<ALIGN>(address + i);
        }
    }

    static This load_from<let VIEW_ALIGN : int>(This* view) {
        return { load_non_writable_plain<VIEW_ALIGN>(__getAddress(view->address)) };
    }
}

struct Buffer<T, let ALIGN : int> : INonWritable {
    T* address;

    __subscript(uint i)->T {
        get {
            return loadAligned<ALIGN>(address + i);
        }
        [nonmutating] set {
            storeAligned<ALIGN>(address + i, newValue);
        }
    }

    T* at(uint i) {
        return address + i;
    }

    static This load_from<let VIEW_ALIGN : int>(This* view) {
        return { load_non_writable_plain<VIEW_ALIGN>(__getAddress(view->address)) };
    }
}

struct NonWritableReference<T : INonWritable, let ALIGN : int> : INonWritable {
    T* address;

    property T value {
        get {
            return load_non_writable<ALIGN>(address);
        }
    }

    static This load_from<let VIEW_ALIGN : int>(This* view) {
        return { load_non_writable_plain<VIEW_ALIGN>(__getAddress(view->address)) };
    }
}

struct Reference<T, let ALIGN : int> {
    T* address;

    property T value {
        get {
            return loadAligned<ALIGN>(address);
        }
    }
}

struct Exchanged<T, let ALIGN : int> : INonWritable {
    T* address;

    __subscript(uint i)->T {
        get {
            T* element = address + i;
            if (ALIGN == 16) {
                return spirv_asm { result:$$T = OpLoad $element Aligned|NonPrivatePointer 16 };
            }
            return spirv_asm { result:$$T = OpLoad $element Aligned|NonPrivatePointer 4 };
        }
        [nonmutating] set {
            T* element = address + i;
            if (ALIGN == 16) {
                spirv_asm { OpStore $element $newValue Aligned|NonPrivatePointer 16 };
            } else {
                spirv_asm { OpStore $element $newValue Aligned|NonPrivatePointer 4 };
            }
        }
    }

    static This load_from<let VIEW_ALIGN : int>(This* view) {
        return { load_non_writable_plain<VIEW_ALIGN>(__getAddress(view->address)) };
    }
}

struct QueueCoherent<T> {
    T* address;

    __subscript(uint i)->T {
        get {
            return loadCoherent<4, MemoryScope.QueueFamily>(address + i);
        }
        [nonmutating] set {
            storeCoherent<4, MemoryScope.QueueFamily>(address + i, newValue);
        }
    }
}

typealias Floats = Buffer<float, 4>;
typealias Uints = Buffer<uint, 4>;
typealias Uvec2s = Buffer<uint2, 8>;
typealias Vec4s = Buffer<float4, 16>;
typealias Uvec4s = Buffer<uint4, 16>;
typealias Ivec4s = Buffer<int4, 16>;

typealias ReadFloats = NonWritableBuffer<float, 4>;
typealias ReadUints = NonWritableBuffer<uint, 4>;
typealias ReadUvec2s = NonWritableBuffer<uint2, 8>;
typealias ReadVec4s = NonWritableBuffer<float4, 16>;
typealias ReadUvec4s = NonWritableBuffer<uint4, 16>;
typealias ReadIvec4s = NonWritableBuffer<int4, 16>;

struct Columns4 : INonWritable {
    row_major float4x4 columns;

    static Columns4 load_from<let ALIGN : int>(Columns4* address) {
        float4* rows = (float4*)address;
        return { float4x4(
            load_non_writable<ALIGN>(rows),
            load_non_writable<ALIGN>(rows + 1),
            load_non_writable<ALIGN>(rows + 2),
            load_non_writable<ALIGN>(rows + 3)) };
    }
}

typealias Mat4s = NonWritableBuffer<Columns4, 16>;

typealias SharedPointer<T> = Ptr<T, Access.ReadWrite, AddressSpace.GroupShared>;

[ForceInline]
T shared_load<T>(SharedPointer<T> element) {
    return loadCoherent<4, MemoryScope.Workgroup>(element);
}

[ForceInline]
void shared_store<T>(SharedPointer<T> element, T value) {
    storeCoherent<4, MemoryScope.Workgroup>(element, value);
}

void exchange_barrier() {
    spirv_asm {
        OpControlBarrier Workgroup Workgroup AcquireRelease|UniformMemory|WorkgroupMemory|MakeAvailable|MakeVisible
    };
}

void buffer_barrier() {
    spirv_asm {
        OpControlBarrier Workgroup Workgroup AcquireRelease|UniformMemory|WorkgroupMemory
    };
}

void group_barrier() {
    spirv_asm {
        OpControlBarrier Workgroup Workgroup AcquireRelease|WorkgroupMemory
    };
}

uint queue_arrive(uint* counter) {
    return spirv_asm {
        result:$$uint = OpAtomicIAdd $counter QueueFamily AcquireRelease|UniformMemory $(1u)
    };
}

void queue_reset(uint* counter) {
    spirv_asm {
        OpAtomicStore $counter QueueFamily None $(0u)
    };
}

uint queue_atomic_max(uint* counter, uint value) {
    return spirv_asm {
        result:$$uint = OpAtomicUMax $counter QueueFamily None $value
    };
}

uint queue_atomic_add(uint* counter, uint value) {
    return spirv_asm {
        result:$$uint = OpAtomicIAdd $counter QueueFamily None $value
    };
}

[ForceInline]
uint shared_atomic_max(SharedPointer<uint> counter, uint value) {
    return spirv_asm {
        result:$$uint = OpAtomicUMax $counter Workgroup None $value
    };
}

[ForceInline]
uint shared_atomic_add(SharedPointer<uint> counter, uint value) {
    return spirv_asm {
        result:$$uint = OpAtomicIAdd $counter Workgroup None $value
    };
}
