#!/bin/sh
echo check >> order.log
amends=$(grep -c '^amend$' order.log)
[ "$amends" -ge 2 ] && exit 0
exit 5
