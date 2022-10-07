// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2022

#include <stdarg.h>
#include <stdio.h>
#include <string.h>


struct buf {
    size_t len;
    char data[120];
};

void buf_reset(struct buf* b) {
    b->len = 0;
}

size_t buf_remaining(const struct buf* b) {
    if (b->len >= sizeof(b->data))
        return 0;
    return sizeof(b->data) - b->len;
}

int buf_bump(struct buf* b, int chars) {
    if (chars > 0)
        b->len += chars;
    return chars;
}

int buf_append(struct buf* b, const char *s) {
    size_t len = strlen(s);
    if (len > buf_remaining(b))
        return 0;
    memcpy(b->data + b->len, s, len);
    return buf_bump(b, len);
}

int buf_printf(struct buf* b, const char *format, ...) {
    size_t rem = buf_remaining(b);
    if (!rem)
        return 0;

    va_list ap;

    va_start(ap, format);
    int retval = vsnprintf(b->data + b->len, rem, format, ap);
    va_end(ap);

    return buf_bump(b, retval);
}


int main() {
}

