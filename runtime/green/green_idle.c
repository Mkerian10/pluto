#include "green_sched.h"
#include "green_chan.h"
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
static void waiter(void *arg){ GChan *c=(GChan*)arg; gchan_recv(c); }  // parks forever
int main(int argc, char **argv){
    long K = argc>1 ? atol(argv[1]) : 100000;
    long stack = argc>2 ? atol(argv[2]) : 16*1024;
    for(long i=0;i<K;i++){ GChan *c=gchan_new(1); green_spawn(waiter,c,stack); }
    green_run();   // returns when the ready queue drains (all parked)
    printf("green_live (parked)=%ld on ONE OS thread (stack=%ld KB each)\n", green_live(), stack/1024);
    printf("ready\n"); fflush(stdout);
    pause();        // stay resident for RSS sampling
    return 0;
}
