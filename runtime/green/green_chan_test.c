#include "green_sched.h"
#include "green_chan.h"
#include <stdio.h>
static GChan *ch; static long N; static long got;
static void producer(void *a){ (void)a; for(long i=0;i<N;i++) gchan_send(ch,1); }
static void consumer(void *a){ (void)a; long s=0; for(long i=0;i<N;i++) s+=gchan_recv(ch); got=s; }
int main(){
    N=1000000; ch=gchan_new(4);
    green_spawn(consumer,0,64*1024);
    green_spawn(producer,0,64*1024);
    green_run();
    printf("green chan producer/consumer: got=%ld expect=%ld %s\n", got, N, got==N?"OK":"FAIL");
    return got==N?0:1;
}
