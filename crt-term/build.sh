#!/bin/bash
cd ~/dev/spixa/crt-term
trunk build --release --public-url /terminal/

rm -rf ~/dev/spixa/static/terminal
mkdir -p ~/dev/spixa/static/terminal
cp -a dist/. ~/dev/spixa/static/terminal/
