#!/bin/bash
cat <<EOF
connect in emacs with:
gdb-multiarch -i=mi target/thumbv6m-none-eabi/debug/macropad-3x3

then:
target extended-remote localhost:3333
monitor reset init
break main
continue
EOF
openocd -f interface/cmsis-dap.cfg -f target/rp2040.cfg -c "adapter speed 5000"
