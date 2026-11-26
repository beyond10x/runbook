#!/bin/sh
# The stand-in of the step that starts long work (a build, a deploy, a pull request's checks): it
# writes the work's record into the workspace and ends at once; the poll step waits for the work.
set -eu
cat > /dev/null
printf 'started\n' > work.txt
echo "work started"
