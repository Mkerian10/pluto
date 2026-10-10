#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
typedef struct GProdTask GProdTask;
extern GProdTask *__pluto_green_prod_spawn(long (*fn)(void *), void *arg, size_t);
extern long __pluto_green_prod_get(GProdTask *);
// Stubs for GC coordination (real impls live in marksweep.c).
static int reg_calls=0, unreg_calls=0, threadreg=0;
void *__pluto_gc_register_green_context(void *top,void *sp){(void)top;(void)sp;__sync_fetch_and_add(&reg_calls,1);return (void*)1;}
void __pluto_gc_unregister_green_context(void *h){(void)h;__sync_fetch_and_add(&unreg_calls,1);}
void __pluto_gc_register_thread_stack(void *l,void *h){(void)l;(void)h;threadreg=1;}
void __pluto_gc_enter_safe_region(void){}
void __pluto_gc_leave_safe_region(void){}
static long work(void *a){ long x=(long)(intptr_t)a; return x*x; }
int main(){
    enum { N=2000 };
    GProdTask *ts[N];
    for (long i=0;i<N;i++) ts[i]=__pluto_green_prod_spawn(work,(void*)(intptr_t)i,64*1024);
    long sum=0, expect=0;
    for (long i=0;i<N;i++){ sum+=__pluto_green_prod_get(ts[i]); expect+=i*i; }
    printf("sum=%ld expect=%ld reg=%d unreg=%d threadreg=%d : %s\n",
           sum,expect,reg_calls,unreg_calls,threadreg,
           (sum==expect && reg_calls==N && unreg_calls==N && threadreg==1)?"OK":"FAIL");
    return (sum==expect && reg_calls==N && unreg_calls==N && threadreg)?0:1;
}
