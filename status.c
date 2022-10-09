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

#include <liburing.h>


#define MAX_STATUS_ENTRIES 16


struct buf {
    uint8_t len;
    uint8_t reserve;
    char data[120];
};

void buf_reset(struct buf* b, uint8_t reserve) {
    b->len = 0;
    b->reserve = reserve;
}

void buf_take_reserve(struct buf* b) {
    b->reserve = 0;
}

size_t buf_remaining(const struct buf* b) {
    if (b->len >= sizeof(b->data))
        return 0;
    return sizeof(b->data) - (b->len + b->reserve);
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

int buf_terminate(struct buf* b) {
    if (1 > buf_remaining(b))
        return -1;
    b->data[b->len] = '\0';
    return buf_bump(b, 1);
}


struct status_entry {
    char* name;
    int fd;
    char* (*extract)(struct buf*);
};


char* extract_psi(struct buf* buf) {
    if (buf_terminate(buf) < 0)
        return "???";

    char* line = strtok(buf->data, "\n");
    while (line) {
        if (strncmp(line, "some", 4) == 0) {
            char* kv = strtok(line, " ");
            while (kv) {
                if (strncmp(kv, "avg10=", 6) == 0)
                    return kv + 6;
                kv = strtok(NULL, " ");
            }
        }
        line = strtok(NULL, "\n");
    }

    return "???";
}


char* extract_word(struct buf* buf) {
    if (buf_terminate(buf) < 0)
        return "???";

    char* word = strtok(buf->data, " ");
    if (word)
        return word;

    return "???";
}


void die(const char* str) {
    fprintf(stderr, "%s: %s\n", str, strerror(errno));
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


int main(int argc, char* argv[]) {
    int res;
    char* config = "";

    struct status_entry entry[16];
    size_t entry_num = 0;

    struct io_uring ring;
    res = io_uring_queue_init(sizeof(entry)/sizeof(entry[0]), &ring, 0);
    if (res >= 0) {
        if (argc >= 2)
            config = argv[1];
    } else {
        fprintf(stderr, "Could not initialize io_uring: %s\n", strerror(-res));
    }

    // We could as well open all these files using the io_uring, but we don't
    // want holes in `entry` if we can't open some of them.
    for (char* c = config; *c; ++c) switch (*c) {
    case 'p': // Pressure
        res = open("/proc/pressure/cpu", O_RDONLY);
        if (res >= 0) {
            struct status_entry* e = entry + entry_num++;
            e->name = "cpu";
            e->fd = res;
            e->extract = extract_psi;
        }

        res = open("/proc/pressure/memory", O_RDONLY);
        if (res >= 0) {
            struct status_entry* e = entry + entry_num++;
            e->name = "mem";
            e->fd = res;
            e->extract = extract_psi;
        }

        res = open("/proc/pressure/io", O_RDONLY);
        if (res >= 0) {
            struct status_entry* e = entry + entry_num++;
            e->name = "io";
            e->fd = res;
            e->extract = extract_psi;
        }
        break;

    case 'l': // Load
        res = open("/proc/loadavg", O_RDONLY);
        if (res >= 0) {
            struct status_entry* e = entry + entry_num++;
            e->name = "load";
            e->fd = res;
            e->extract = extract_word;
        }
        break;

    default:
        fprintf(stderr, "Specifier not recognized: %c\n", *c);
        exit(1);
    }

    int timer = timerfd_create(CLOCK_REALTIME, 0);
    if (timer < 0)
        die("Could create timer");
    if (arm_timer(timer) < 0)
        die("Could not arm timer");

    while (1) {
        struct buf line;
        buf_reset(&line, 1); // We reserve one byte for the newline

        // Prepare reads for each entry in advance
        struct buf entry_rawdata[sizeof(entry)/sizeof(entry[0])];
        for (size_t i = 0; i<entry_num; ++i) {
            int fd = entry[i].fd;

            struct buf* buf = entry_rawdata + i;
            // We may need to add a null-byte at some point after the read
            buf_reset(buf, 1);

            struct io_uring_sqe* sqe = io_uring_get_sqe(&ring);
            if (!sqe)
                break;
            io_uring_prep_read(sqe, fd, buf->data, buf_remaining(buf), 0);
            io_uring_sqe_set_data64(sqe, i);
        }

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

        int submitted = io_uring_submit(&ring);

        // Current time
        {
            size_t len = buf_remaining(&line);
            time_t t = time(NULL);
            len = strftime(line.data, len, "%F %T", localtime(&t));
            buf_bump(&line, len);
        }

        // Process submission results, collecting vals to include in the output
        char* strdata[sizeof(entry)/sizeof(entry[0])];
        while (submitted-- > 0) {
            struct io_uring_cqe* cqe;
            if (io_uring_wait_cqe(&ring, &cqe) < 0)
                break;

            uint64_t i = io_uring_cqe_get_data64(cqe);

            struct buf* buf = entry_rawdata + i;
            buf_bump(buf, cqe->res);
            buf_take_reserve(buf);
            io_uring_cqe_seen(&ring, cqe);

            strdata[i] = (entry[i].extract)(entry_rawdata + i);
        }

        // Extend line with values
        for (size_t i = 0; i<entry_num; ++i) {
            const char* name = entry[i].name;
            const char* val = strdata[i];
            if (name)
                buf_printf(&line, " %s: %s", name, val);
            else
                buf_printf(&line, " %s", val);
        }

        // The newline is the one thing which has to end up in the line. Without
        // it, there's no point in printing the buffer's contents.
        buf_take_reserve(&line);
        if (buf_append(&line, "\n") <= 0)
            continue;
        res = write(STDOUT_FILENO, line.data, line.len);
        if (res < 0)
            die("Could not write to stdout");
        if (res == 0)
            exit(0);
    }
}

