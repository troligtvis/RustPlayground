#ifndef XI_CORE_H
#define XI_CORE_H
typedef struct _XiCore XiCore;

typedef struct _XiLine {
    char *text;
    int32_t cursor;
    int32_t selection[2];
} XiLine;

typedef const char* json;

typedef void (*rpc_callback)(json);
typedef void (*invalidate_callback)(size_t start, size_t end);

extern XiCore* xiCoreCreate(rpc_callback, invalidate_callback);
extern void xiCoreFree(XiCore*);
extern void xiCoreSendMessage(XiCore*, json);
extern XiLine* xiCoreGetLine(XiCore*, uint32_t);

#endif