/* hotpath service harness for the cjson target (SPEC §3.1 service mode).
 *
 * TRUST-LAYER CODE — outside targets/cjson/workspace, compiled with the
 * (patched) workspace cJSON.c, so a patch changes the parser under HTTP
 * load but never this server. A minimal single-threaded HTTP/1.1 server:
 * each request parses+serializes a fixed JSON document `iters` times
 * (deterministic CPU service time) and replies with the checksum. Single
 * worker by design — the cleanest, least-noisy service-time signal; the
 * coordinated-omission-correct load generator captures the queueing when
 * arrival rate approaches the worker's capacity.
 *
 *   cjson-service <port> <doc.json> <iters-per-request>
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <sys/socket.h>
#include "cJSON.h"

#include <time.h>

static char *g_doc;
static long g_inject_ns = 0; /* calibration: busy-wait per request (HOTPATH_INJECT_US) */

static void busy_wait_ns(long ns) {
    if (ns <= 0) return;
    struct timespec s;
    clock_gettime(CLOCK_MONOTONIC, &s);
    long long target = (long long)s.tv_sec * 1000000000LL + s.tv_nsec + ns;
    for (;;) {
        struct timespec n;
        clock_gettime(CLOCK_MONOTONIC, &n);
        if ((long long)n.tv_sec * 1000000000LL + n.tv_nsec >= target) break;
    }
}

static unsigned long do_work(long iters) {
    unsigned long acc = 1469598103934665603UL;
    busy_wait_ns(g_inject_ns);
    for (long i = 0; i < iters; i++) {
        cJSON *root = cJSON_Parse(g_doc);
        if (!root) return 0;
        char *out = cJSON_PrintUnformatted(root);
        if (out) {
            for (char *p = out; *p; p++) acc = (acc ^ (unsigned char)*p) * 1099511628211UL;
            free(out);
        }
        cJSON_Delete(root);
    }
    return acc;
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: %s <port> <doc.json> <iters>\n", argv[0]);
        return 2;
    }
    int port = atoi(argv[1]);
    long iters = atol(argv[3]);
    if (iters < 1) iters = 1;
    /* Calibration-only: inject a fixed per-request busy-wait to validate
     * the load generator detects a known latency regression. Off in every
     * real run (unset). */
    const char *inj = getenv("HOTPATH_INJECT_US");
    if (inj) g_inject_ns = atol(inj) * 1000L;

    FILE *f = fopen(argv[2], "rb");
    if (!f) { perror("fopen"); return 2; }
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    g_doc = (char *)malloc((size_t)n + 1);
    if (fread(g_doc, 1, (size_t)n, f) != (size_t)n) { return 2; }
    g_doc[n] = '\0';
    fclose(f);

    int srv = socket(AF_INET, SOCK_STREAM, 0);
    if (srv < 0) { perror("socket"); return 2; }
    int one = 1;
    setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = htons((unsigned short)port);
    if (bind(srv, (struct sockaddr *)&addr, sizeof(addr)) < 0) { perror("bind"); return 2; }
    if (listen(srv, 1024) < 0) { perror("listen"); return 2; }

    /* Signal readiness on stdout so the harness knows the port is up. */
    printf("READY %d\n", port);
    fflush(stdout);

    char reqbuf[8192];
    for (;;) {
        int c = accept(srv, NULL, NULL);
        if (c < 0) continue;
        setsockopt(c, IPPROTO_TCP, TCP_NODELAY, &one, sizeof(one));
        /* Drain the request line + headers (we ignore them). */
        ssize_t r = recv(c, reqbuf, sizeof(reqbuf) - 1, 0);
        if (r <= 0) { close(c); continue; }

        unsigned long acc = do_work(iters);

        char body[64];
        int blen = snprintf(body, sizeof(body), "checksum=%lu\n", acc);
        char resp[256];
        int rlen = snprintf(resp, sizeof(resp),
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n"
                            "Content-Length: %d\r\nConnection: close\r\n\r\n%s",
                            blen, body);
        ssize_t off = 0;
        while (off < rlen) {
            ssize_t w = send(c, resp + off, (size_t)(rlen - off), 0);
            if (w <= 0) break;
            off += w;
        }
        close(c);
    }
    return 0;
}
