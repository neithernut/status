// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2022

#include <errno.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <sys/timerfd.h>


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


void die(const char* str) {
    fprintf(stderr, "%s: %s", str, strerror(errno));
    exit(1);
}


int arm_timer(int fd) {
    int res;
    do {
        // We try to configure our timer to trigger on exact wallclock seconds.
        struct itimerspec config = {
            .it_interval = {0, 500000000}
        };
        res = clock_gettime(CLOCK_REALTIME, &config.it_value);
        if (res < 0)
            break;
        config.it_value.tv_nsec = 0;
        res = timerfd_settime(fd, TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET, &config, NULL);
    } while (res < 0 && errno == ECANCELED);
    return res;
}


int main() {
    int res;

    int timer = timerfd_create(CLOCK_REALTIME, 0);
    if (timer < 0)
        die("Could create timer");
    if (arm_timer(timer) < 0)
        die("Could not arm timer");

    while (1) {
        struct buf line;
        buf_reset(&line);

        {
            uint64_t buf;
            if (read(timer, &buf, sizeof(buf)) < 0) {
                if (errno == ECANCELED) {
                    if (arm_timer(timer) < 0)
                        die("Could not rearm timer");
                    continue;
                }
                die("Broken timer");
            }
        }

        // TODO: contents

        // The newline is the one thing which has to end up in the line. Without
        // it, there's no point in printing the buffer's contents.
        if (buf_append(&line, "\n") < 0)
            continue;
        res = write(STDOUT_FILENO, line.data, line.len);
        if (res < 0)
            die("Could not write to stdout");
        if (res == 0)
            exit(0);
    }
}

