#include "green_sched.h"
#include <stdio.h>
#include <stdint.h>
#include <time.h>
static uint64_t ns(){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (uint64_t)t.tv_sec*1000000000ull+t.tv_nsec;}
static volatile long counter;
static long YIELDS;
static void task(void *arg){ long id=(long)arg; (void)id; for(long i=0;i<YIELDS;i++){ counter++; green_yield(); } }
int main(){
    long N=10000; YIELDS=100;
    uint64_t t0=ns();
    for(long i=0;i<N;i++) green_spawn(task,(void*)i, 64*1024);
    green_run();
    uint64_t t1=ns();
    long expect=N*YIELDS;
    long switches=counter*2; // ~2 switches per yield round
    printf("green tasks=%ld yields/task=%ld counter=%ld (expect %ld) %s\n",N,YIELDS,counter,expect, counter==expect?"OK":"FAIL");
    printf("total %.1f ms, %.0f ns / yield-roundtrip, %.2f M switches/s\n",(t1-t0)/1e6,(double)(t1-t0)/counter,(double)switches/((t1-t0)/1e9)/1e6);
    return 0;
}
