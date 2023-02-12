
#!/bin/sh
if [[ $# -ne 1 ]]
then
  echo "Usage:"
  echo "touf2.sh <output filename>"
  echo
  echo "Note that you must have 'uf2conv.py' executable in your path."
  exit -1
fi
cargo objcopy --release -- -O binary "$1".bin
# N.B. 0x26000 is for the S140 v6, use 0x27000 for the S140 v7
uf2conv.py "$1".bin -c -f 0xADA52840 -b 0x26000 -o "$1".uf2
