# prog1: exec target. Touches an unmapped page in the demand region to prove
# demand paging, verifies the stored value, prints a string, and exits.
li x4, 0x900000         # unmapped VA in the demand region (PD index 4)
li x5, 'Q'
store64 x4, x5, 0       # page fault: kernel maps the page, then retries
load64 x5, x4, 0
addi x2, x5, 0          # x2 = value read back
trap 1                  # prints 'Q'
li x4, msg
ploop:
load8 x2, x4, 0
beq x2, x0, pdone
trap 1
addi x4, x4, 1
jmp ploop
pdone:
trap 5                  # exit
msg:
.string "D"
