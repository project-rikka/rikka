import cloth.common;

static const uint STRAND = 0x80000000u;

struct Bind : INonWritable {
    uint4 header;
    uint4 target[10];

    static Bind load_from<let ALIGN : int>(Bind* address) {
        Bind bind;
        bind.header = load_non_writable<16>(__getAddress(address->header));
        [ForceUnroll]
        for (uint k = 0u; k < 10u; ++k) {
            bind.target[k] = load_non_writable<16>(__getAddress(address->target[k]));
        }
        return bind;
    }
};

typealias Binds = NonWritableBuffer<Bind, 16>;
