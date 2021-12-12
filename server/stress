#!/bin/bash

# trap ctrl-c and call ctrl_c()
trap end INT

function end() {
    printf "\n" > /tmp/srv-input
    rm -f /tmp/srv-input
    sleep 0.5
    exit 0
}

docker image load -i mqtt-stresser.tar
sed -i "s/^ip=.*/ip=localhost/" config.txt
rm -f /tmp/srv-input
mkfifo /tmp/srv-input
cargo build --release
printf "Ejecutando stress test con flags:\n\033[2;37m$SFLAGS\033[0m\n"
cat /tmp/srv-input | cargo run --release > /dev/null &
docker run --net=host --rm inovex/mqtt-stresser -broker tcp://localhost:1883 \
    -username fdelu -password fdelu $SFLAGS
end