class MemoryView {
    constructor(memory, type, length, byteLength, byteOffset) {
        this.memory = memory;
        this.type = type;
        this.length = length;
        this.byteLength = byteLength;
        this.byteOffset = byteOffset;
        this._view = null;
    }

    get view() {
        return this._view ??= new this.type(this.memory.buffer, this.byteOffset, this.length);
    }

    get(index) {
        return this.view[index];
    }
}

class WasmPageAllocator {
    constructor(memory) {
        this.memory = memory;
        this.allocs = 0;
    }

    _typedArray(type, length) {
        const byteLength = length * type.BYTES_PER_ELEMENT;
        const byteOffset = this.memory.buffer.byteLength + this.allocs;
        this.allocs += byteLength;

        const view = new MemoryView(this.memory, type, length, byteLength, byteOffset);
        return view;
    }

    u32(n=1) { return this._typedArray(Uint32Array, n); }

    u8(n=1) { return this._typedArray(Uint8Array, n); }

    reserve() {
        allocateMemory(this.memory, this.allocs);
        this.allocs = 0;
    }
}

function allocateMemory(memory, numBytes) {
    const bytesPerPage = (64 * 1024);
    const numPages = Math.ceil(numBytes / bytesPerPage);
    const start = memory.buffer.byteLength;
    memory.grow(numPages);
    return { byteOffset: start, count: numPages * bytesPerPage };
}
