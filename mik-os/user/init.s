# init: first user process. Prints 'I', forks; the child prints 'C' then
# execs prog1; the parent prints 'P', yields, prints 'E', and exits.
# Register use: x2 = syscall arg/return; x1,x3,x10,x11,x15 are kernel-clobbered.
li x2, 'I'
trap 1
trap 2                  # fork: parent gets x2 = child pid, child gets x2 = 0
bne x2, x0, parent

# --- child ---
li x2, 'C'
trap 1
li x2, 1
trap 3                  # exec program 1 (does not return)

# --- parent ---
parent:
li x2, 'P'
trap 1
trap 4                  # yield until the child finishes
trap 4
trap 4
trap 4
li x2, 'E'
trap 1
trap 5                  # exit
