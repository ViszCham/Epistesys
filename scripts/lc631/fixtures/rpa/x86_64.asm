section .text
global checked_add

checked_add:
    mov rax, rdi
    add rax, rsi
    ret
