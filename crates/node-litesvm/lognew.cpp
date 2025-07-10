#include <new>
#include <cstdio>
#include <cstdlib>

extern "C" void* malloc(size_t);

void* operator new(std::size_t sz) {
    if (sz > 64 * 1024 * 1024)   // log anything >64 MiB
        fprintf(stderr, "new(%zu)\n", sz);
    void* p = std::malloc(sz);
    if (!p) throw std::bad_alloc();
    return p;
}
void operator delete(void* p) noexcept { std::free(p); }
