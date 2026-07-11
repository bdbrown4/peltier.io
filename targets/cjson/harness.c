/* hotpath bench/golden driver for the cjson target.
 *
 * TRUST-LAYER CODE — lives outside targets/cjson/workspace, so the agent's
 * propose_patch allowlist (workspace-relative paths only) cannot touch it.
 * It is compiled together with the (patched) workspace cJSON.c, so a patch
 * changes the parser/printer under test but never this measurement rig.
 *
 * Workload: parse a JSON document `iters` times, serialize it unformatted
 * each time, and fold every serialized byte into a rolling checksum so the
 * optimizer cannot elide the work. Deterministic: the last serialization
 * plus the checksum are written to stdout, giving golden-replay a
 * byte-exact signal over both the parse AND the print path.
 *
 *   cjson-bench <file.json> [iters=40]
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "cJSON.h"

/* coz causal profiling (SPEC §12): compile with -DHOTPATH_COZ and link
 * libcoz to mark each parse+print iteration as a throughput progress
 * point, so `coz run` can measure which source lines, if sped up, would
 * actually raise end-to-end throughput. A no-op in normal builds. */
#ifdef HOTPATH_COZ
#include <coz.h>
#define HOTPATH_PROGRESS() COZ_PROGRESS
#else
#define HOTPATH_PROGRESS() ((void)0)
#endif

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s <file.json> [iters]\n", argv[0]);
        return 2;
    }
    long iters = (argc >= 3) ? atol(argv[2]) : 40;
    if (iters < 1) iters = 1;

    FILE *f = fopen(argv[1], "rb");
    if (!f) { perror("fopen"); return 2; }
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (n < 0) { fclose(f); return 2; }
    char *buf = (char *)malloc((size_t)n + 1);
    if (!buf) { fclose(f); return 2; }
    if (fread(buf, 1, (size_t)n, f) != (size_t)n) { free(buf); fclose(f); return 2; }
    buf[n] = '\0';
    fclose(f);

    unsigned long acc = 1469598103934665603UL; /* FNV-ish rolling checksum */
    char *last = NULL;
    for (long i = 0; i < iters; i++) {
        cJSON *root = cJSON_Parse(buf);
        if (!root) { fprintf(stderr, "parse failed\n"); free(buf); return 3; }
        char *out = cJSON_PrintUnformatted(root);
        if (!out) { cJSON_Delete(root); free(buf); return 3; }
        for (char *p = out; *p; p++) acc = (acc ^ (unsigned char)*p) * 1099511628211UL;
        if (i == iters - 1) last = out;
        else free(out);
        cJSON_Delete(root);
        HOTPATH_PROGRESS(); /* one parse+print = one throughput unit (coz) */
    }

    if (last) {
        fwrite(last, 1, strlen(last), stdout);
        free(last);
    }
    printf("\nchecksum=%lu\n", acc);
    free(buf);
    return 0;
}
