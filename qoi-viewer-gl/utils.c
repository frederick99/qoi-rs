
#define ARRAY_LEN(array) (sizeof(array) / sizeof(*(array)))

#ifndef SHADER_VERSION
#   define SHADER_VERSION "#version 330 core"
#endif
#define SHADER_SRC(...) SHADER_VERSION "\n" #__VA_ARGS__

typedef struct {
    char* data;
    size_t count;
    size_t capacity;
} StringBuffer;

void sb_extend(StringBuffer* sb, char* src, size_t count) {
    size_t new_count = sb->count + count;
    if (new_count > sb->capacity) {
        if (sb->capacity == 0) sb->capacity = 256;
        while (new_count > sb->capacity) sb->capacity *= 2;
        sb->data = realloc(sb->data, sb->capacity);
        if (!sb->data) {
            printf("Realloc failed.\n");
            abort();
        }
    }

    memcpy(sb->data + sb->count, src, count);
    sb->count = new_count;
}

bool read_entire_file(const char* file_path, StringBuffer* sb) {
    FILE* file = fopen(file_path, "rb");
    if (!file) return false;

    size_t n;
    char buf[1024];
    while (n = fread(buf, 1, sizeof(buf), file)) sb_extend(sb, buf, n);

    fclose(file);
    return true;
}

typedef struct {
    union {
        struct { float x, y; };
        struct { float s, t; };
        struct { float u, v; };
    };
} Vector2;

typedef struct {
    union {
        struct { float x, y, z; };
        struct { float r, g, b; };
    };
} Vector3;

typedef struct {
    Vector3 pos;
    Vector3 col;
    Vector2 tex;
} Vertex;

typedef struct {
    float r0;
    float r1;
    float r2;
    float r3;
} Column;

typedef struct {
    Column c0;
    Column c1;
    Column c2;
    Column c3;
} Matrix;

void matrix_load_identity(Matrix* m) {
    *m = (Matrix) {0};
    m->c0.r0 = 1.0;
    m->c1.r1 = 1.0;
    m->c2.r2 = 1.0;
    m->c3.r3 = 1.0;
}

void matrix_scale(Matrix* m, float x, float y, float z) {
    m->c0.r0 *= x;
    m->c1.r1 *= y;
    m->c2.r2 *= z;
}

void matrix_translate(Matrix* m, float x, float y, float z) {
    m->c3.r0 += x;
    m->c3.r1 += y;
    m->c3.r2 += z;
}
