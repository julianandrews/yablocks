#!/usr/bin/env sh
#
# Sample click handler for i3bar.

handle_volume_click() {
  case "$1" in
    1)
      pactl set-sink-volume @DEFAULT_SINK@ -5% ;;
    2)
      pactl set-sink-mute @DEFAULT_SINK@ toggle ;;
    3)
      pactl set-sink-volume @DEFAULT_SINK@ +5% ;;
    4)
      pactl set-sink-volume @DEFAULT_SINK@ +1% ;;
    5)
      pactl set-sink-volume @DEFAULT_SINK@ -1% ;;
  esac
}

jq -c --unbuffered --stream 'fromstream(1|truncate_stream(inputs))' </dev/stdin | while read -r line; do
  name=$(echo "$line" | jq -cr '.name')
  if [ "$name" = "volume" ]; then
    button=$(echo "$line" | jq -cr '.button')
    handle_volume_click "$button"
  fi
done
