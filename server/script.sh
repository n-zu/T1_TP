#!/usr/bin/zsh
for i in {1..50}
do
       cargo test >> res.txt
done
