#!/usr/bin/env python3
"""
ID-Cooling FX360 LCD Controller for Linux

Reverse-engineered protocol for controlling the 240x240 LCD on
ID-Cooling FX360 (and possibly other ID-Cooling LCD coolers).

USB Device: 2000:3000 (CMX Systems HOTSPOTEKUSB HID DEMO)
Protocol: JPEG images sent via USB HID interrupt transfers

Usage:
    # Display a static image
    python idcooling_lcd.py image photo.png

    # Display a solid color
    python idcooling_lcd.py color red
    python idcooling_lcd.py color "#FF6600"
    python idcooling_lcd.py color 255,128,0

    # Display a GIF animation
    python idcooling_lcd.py gif animation.gif

    # Display system stats (CPU temp, usage, etc.)
    python idcooling_lcd.py sysinfo

    # Display currently playing track (MPRIS/D-Bus)
    python idcooling_lcd.py nowplaying
    python idcooling_lcd.py nowplaying --player spotify

Requires: pyusb, Pillow
Optional: dbus-python (for nowplaying mode)
"""

import argparse
import io
import math
import os
import signal
import struct
import sys
import time
import threading

try:
    import usb.core
    import usb.util
except ImportError:
    print("ERROR: pyusb is required. Install with: pip install pyusb")
    sys.exit(1)

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("ERROR: Pillow is required. Install with: pip install Pillow")
    sys.exit(1)

try:
    import dbus
    HAS_DBUS = True
except ImportError:
    HAS_DBUS = False

from urllib.parse import unquote, urlparse
from urllib.request import urlopen


# ==============================================================================
# Protocol Constants
# ==============================================================================

VENDOR_ID = 0x2000
PRODUCT_ID = 0x3000

LCD_WIDTH = 240
LCD_HEIGHT = 240

PACKET_SIZE = 1024
HEADER_SIZE = 32
JPEG_FIRST_CHUNK = PACKET_SIZE - HEADER_SIZE  # 992 bytes

# Magic bytes
MAGIC = b'CRT\x00\x00'

# Commands
CMD_DRAW = b'DRA\x00'
CMD_CONNECT = b'CONNECT'

# Unknown constant flag (always 0xB1 in captures)
FLAG_BYTE = 0xB1

# Timing
TARGET_FPS = 20
FRAME_INTERVAL = 1.0 / TARGET_FPS  # 50ms
KEEPALIVE_INTERVAL = 8.0  # seconds

# JPEG quality for encoding
JPEG_QUALITY = 85


# ==============================================================================
# Protocol Implementation
# ==============================================================================

def build_draw_header(jpeg_size: int) -> bytes:
    """Build the 32-byte DRA header for an image frame.

    Header layout:
        [0-3]   "CRT\\0"     Magic
        [4]     0x00          Separator
        [5-8]   "DRA\\0"     Draw command
        [9-11]  uint24 BE    Total frame size (jpeg_size + 32)
        [12]    0xB1         Flag byte
        [13-31] zeros        Padding
    """
    total_size = jpeg_size + HEADER_SIZE
    size_bytes = total_size.to_bytes(3, byteorder='big')

    header = bytearray(HEADER_SIZE)
    header[0:5] = MAGIC
    header[5:9] = CMD_DRAW
    header[9:12] = size_bytes
    header[12] = FLAG_BYTE
    # Bytes 13-31 remain zero

    return bytes(header)


def build_connect_packet() -> bytes:
    """Build the CONNECT keepalive packet (1024 bytes)."""
    packet = bytearray(PACKET_SIZE)
    packet[0:5] = MAGIC
    packet[5:12] = CMD_CONNECT
    return bytes(packet)


def image_to_jpeg(image: Image.Image, quality: int = JPEG_QUALITY) -> bytes:
    """Convert a PIL Image to JPEG bytes, resizing to LCD dimensions."""
    # Resize to LCD resolution, maintaining aspect ratio with center crop
    img = image.convert('RGB')

    # Calculate crop for center-fill
    src_ratio = img.width / img.height
    dst_ratio = LCD_WIDTH / LCD_HEIGHT

    if src_ratio > dst_ratio:
        # Source is wider - crop sides
        new_width = int(img.height * dst_ratio)
        left = (img.width - new_width) // 2
        img = img.crop((left, 0, left + new_width, img.height))
    elif src_ratio < dst_ratio:
        # Source is taller - crop top/bottom
        new_height = int(img.width / dst_ratio)
        top = (img.height - new_height) // 2
        img = img.crop((0, top, img.width, top + new_height))

    img = img.resize((LCD_WIDTH, LCD_HEIGHT), Image.LANCZOS)

    # The LCD is mounted upside-down, so rotate 180°
    img = img.rotate(180)

    # Encode as JPEG
    buf = io.BytesIO()
    img.save(buf, format='JPEG', quality=quality)
    return buf.getvalue()


def build_frame_packets(jpeg_data: bytes) -> list[bytes]:
    """Split a JPEG image into protocol packets ready for USB transfer.

    Returns a list of 1024-byte packets:
        - First packet: 32-byte header + up to 992 bytes of JPEG
        - Subsequent packets: 1024 bytes of JPEG continuation (zero-padded)
    """
    header = build_draw_header(len(jpeg_data))
    packets = []

    # First packet: header + start of JPEG
    first_packet = bytearray(PACKET_SIZE)
    first_packet[0:HEADER_SIZE] = header
    chunk = jpeg_data[:JPEG_FIRST_CHUNK]
    first_packet[HEADER_SIZE:HEADER_SIZE + len(chunk)] = chunk
    packets.append(bytes(first_packet))

    # Continuation packets
    offset = JPEG_FIRST_CHUNK
    while offset < len(jpeg_data):
        pkt = bytearray(PACKET_SIZE)
        chunk = jpeg_data[offset:offset + PACKET_SIZE]
        pkt[:len(chunk)] = chunk
        packets.append(bytes(pkt))
        offset += PACKET_SIZE

    return packets


# ==============================================================================
# USB Communication
# ==============================================================================

class IDCoolingLCD:
    """Interface to the ID-Cooling FX360 LCD over USB."""

    def __init__(self):
        self.device = None
        self.endpoint_out = None
        self._stop_event = threading.Event()
        self._last_keepalive = 0

    def connect(self) -> bool:
        """Find and connect to the cooler LCD."""
        self.device = usb.core.find(idVendor=VENDOR_ID, idProduct=PRODUCT_ID)

        if self.device is None:
            print(f"ERROR: Device {VENDOR_ID:04x}:{PRODUCT_ID:04x} not found.")
            print("Make sure the cooler is connected via USB.")
            return False

        # Detach kernel driver if necessary
        try:
            if self.device.is_kernel_driver_active(0):
                self.device.detach_kernel_driver(0)
                print("Detached kernel HID driver.")
        except (usb.core.USBError, NotImplementedError):
            pass

        # Set configuration
        try:
            self.device.set_configuration()
        except usb.core.USBError:
            pass  # May already be configured

        # Find the OUT endpoint (EP1 OUT)
        cfg = self.device.get_active_configuration()
        intf = cfg[(0, 0)]

        self.endpoint_out = usb.util.find_descriptor(
            intf,
            custom_match=lambda e: usb.util.endpoint_direction(e.bEndpointAddress)
            == usb.util.ENDPOINT_OUT
        )

        if self.endpoint_out is None:
            print("ERROR: Could not find OUT endpoint.")
            return False

        print(f"Connected to ID-Cooling LCD (EP 0x{self.endpoint_out.bEndpointAddress:02x})")
        return True

    def disconnect(self):
        """Release the USB device."""
        self._stop_event.set()
        if self.device is not None:
            try:
                usb.util.dispose_resources(self.device)
            except Exception:
                pass

    def send_packet(self, data: bytes):
        """Send a single 1024-byte packet to the device."""
        assert len(data) == PACKET_SIZE, f"Packet must be {PACKET_SIZE} bytes, got {len(data)}"
        self.endpoint_out.write(data)

    def send_frame(self, jpeg_data: bytes):
        """Send a complete JPEG frame to the LCD."""
        packets = build_frame_packets(jpeg_data)
        for pkt in packets:
            self.send_packet(pkt)

    def send_keepalive(self):
        """Send a CONNECT keepalive packet."""
        self.send_packet(build_connect_packet())
        self._last_keepalive = time.monotonic()

    def send_image(self, image: Image.Image, quality: int = JPEG_QUALITY):
        """Convert and send a PIL Image to the LCD."""
        jpeg = image_to_jpeg(image, quality)
        self.send_frame(jpeg)

    def display_static(self, image: Image.Image, duration: float = None):
        """Display a static image, sending keepalives as needed.

        Args:
            image: PIL Image to display
            duration: How long to display (None = until interrupted)
        """
        jpeg = image_to_jpeg(image)
        start = time.monotonic()
        frame_count = 0

        try:
            while not self._stop_event.is_set():
                self.send_frame(jpeg)
                frame_count += 1

                # Check keepalive
                if time.monotonic() - self._last_keepalive > KEEPALIVE_INTERVAL:
                    self.send_keepalive()

                # Check duration
                if duration and (time.monotonic() - start) >= duration:
                    break

                time.sleep(FRAME_INTERVAL)

        except KeyboardInterrupt:
            pass

        elapsed = time.monotonic() - start
        if elapsed > 0:
            print(f"\nSent {frame_count} frames in {elapsed:.1f}s ({frame_count/elapsed:.1f} FPS)")

    def display_animation(self, frames: list[Image.Image], fps: float = 20,
                          loop: bool = True):
        """Display an animation (e.g., from a GIF).

        Args:
            frames: List of PIL Images
            fps: Target frames per second
            loop: Whether to loop the animation
        """
        # Pre-encode all frames as JPEG
        jpeg_frames = [image_to_jpeg(f) for f in frames]
        frame_interval = 1.0 / fps
        frame_idx = 0

        print(f"Playing {len(jpeg_frames)} frames at {fps} FPS (loop={'on' if loop else 'off'})")

        try:
            while not self._stop_event.is_set():
                self.send_frame(jpeg_frames[frame_idx])
                frame_idx += 1

                if frame_idx >= len(jpeg_frames):
                    if loop:
                        frame_idx = 0
                    else:
                        break

                # Keepalive
                if time.monotonic() - self._last_keepalive > KEEPALIVE_INTERVAL:
                    self.send_keepalive()

                time.sleep(frame_interval)

        except KeyboardInterrupt:
            pass

    def stop(self):
        """Signal any running display loop to stop."""
        self._stop_event.set()


# ==============================================================================
# Image Generators
# ==============================================================================

def make_solid_color(color) -> Image.Image:
    """Create a solid color image.

    Args:
        color: Can be a color name ("red"), hex ("#FF0000"), or RGB tuple
    """
    if isinstance(color, str):
        if ',' in color:
            parts = [int(x.strip()) for x in color.split(',')]
            color = tuple(parts[:3])
    return Image.new('RGB', (LCD_WIDTH, LCD_HEIGHT), color)


def load_gif_frames(path: str) -> tuple[list[Image.Image], float]:
    """Load frames from a GIF file.

    Returns:
        Tuple of (list of PIL Images, fps)
    """
    gif = Image.open(path)
    frames = []
    durations = []

    try:
        while True:
            frame = gif.copy().convert('RGB')
            frames.append(frame)
            duration = gif.info.get('duration', 50)
            durations.append(duration)
            gif.seek(gif.tell() + 1)
    except EOFError:
        pass

    avg_duration = sum(durations) / len(durations) if durations else 50
    fps = 1000.0 / avg_duration if avg_duration > 0 else 20
    fps = min(fps, TARGET_FPS)  # Cap at device max

    return frames, fps


def make_sysinfo_frame() -> Image.Image:
    """Generate a system info display frame."""
    img = Image.new('RGB', (LCD_WIDTH, LCD_HEIGHT), (0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Try to get system info
    cpu_temp = "N/A"
    cpu_usage = "N/A"
    mem_usage = "N/A"

    # CPU temperature
    try:
        for zone_path in ['/sys/class/thermal/thermal_zone0/temp',
                          '/sys/class/hwmon/hwmon0/temp1_input']:
            if os.path.exists(zone_path):
                with open(zone_path) as f:
                    temp = int(f.read().strip()) / 1000
                cpu_temp = f"{temp:.0f}°C"
                break
    except Exception:
        pass

    # CPU usage
    try:
        with open('/proc/stat') as f:
            line = f.readline()
            parts = line.split()
            idle = int(parts[4])
            total = sum(int(p) for p in parts[1:])
            # Store for delta calculation (rough estimate)
            usage = max(0, min(100, 100 - (idle * 100 / total)))
            cpu_usage = f"{usage:.0f}%"
    except Exception:
        pass

    # Memory usage
    try:
        with open('/proc/meminfo') as f:
            lines = f.readlines()
            meminfo = {}
            for line in lines:
                parts = line.split()
                meminfo[parts[0].rstrip(':')] = int(parts[1])
            total = meminfo.get('MemTotal', 0)
            available = meminfo.get('MemAvailable', 0)
            if total > 0:
                used_pct = (total - available) / total * 100
                used_gb = (total - available) / 1024 / 1024
                total_gb = total / 1024 / 1024
                mem_usage = f"{used_gb:.1f}/{total_gb:.0f}G"
    except Exception:
        pass

    # Draw the display
    try:
        font_large = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 28)
        font_med = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 20)
        font_small = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 16)
    except OSError:
        font_large = ImageFont.load_default()
        font_med = font_large
        font_small = font_large

    # Title
    draw.text((LCD_WIDTH // 2, 20), "SYSTEM", fill=(0, 180, 255),
              font=font_large, anchor="mt")

    # CPU Temp (big, centered)
    y = 65
    draw.text((LCD_WIDTH // 2, y), "CPU TEMP", fill=(150, 150, 150),
              font=font_small, anchor="mt")
    draw.text((LCD_WIDTH // 2, y + 25), cpu_temp, fill=(255, 80, 60),
              font=font_large, anchor="mt")

    # CPU Usage
    y = 130
    draw.text((LCD_WIDTH // 2, y), "CPU USAGE", fill=(150, 150, 150),
              font=font_small, anchor="mt")
    draw.text((LCD_WIDTH // 2, y + 25), cpu_usage, fill=(80, 255, 80),
              font=font_med, anchor="mt")

    # Memory
    y = 185
    draw.text((LCD_WIDTH // 2, y), "MEMORY", fill=(150, 150, 150),
              font=font_small, anchor="mt")
    draw.text((LCD_WIDTH // 2, y + 25), mem_usage, fill=(255, 200, 50),
              font=font_med, anchor="mt")

    return img


# ==============================================================================
# MPRIS Now Playing
# ==============================================================================

SPOTIFY_GREEN = (29, 185, 84)
PROGRESS_BG = (60, 60, 60)
ART_SIZE = 130
ART_MARGIN_TOP = 12


class MPRISClient:
    """Query currently playing track info via MPRIS D-Bus."""

    MPRIS_PREFIX = 'org.mpris.MediaPlayer2.'
    MPRIS_PATH = '/org/mpris/MediaPlayer2'
    PLAYER_IFACE = 'org.mpris.MediaPlayer2.Player'
    PROPS_IFACE = 'org.freedesktop.DBus.Properties'

    def __init__(self, player_name: str = None):
        """Initialize MPRIS client.

        Args:
            player_name: Specific player to target (e.g. 'spotify', 'firefox').
                         If None, uses the first active MPRIS player found.
        """
        self._preferred_player = player_name
        self._bus = None

    def _get_bus(self):
        if self._bus is None:
            self._bus = dbus.SessionBus()
        return self._bus

    def _find_player(self) -> str | None:
        """Find an active MPRIS player on the session bus."""
        bus = self._get_bus()
        names = [str(n) for n in bus.list_names() if n.startswith(self.MPRIS_PREFIX)]

        if not names:
            return None

        if self._preferred_player:
            # Match preferred player (case-insensitive partial match)
            target = self._preferred_player.lower()
            for name in names:
                suffix = name[len(self.MPRIS_PREFIX):].lower()
                if target in suffix:
                    return name

        # Return first available player
        return names[0]

    def get_now_playing(self) -> dict | None:
        """Get current playback info.

        Returns a dict with keys:
            title, artist, album, art_path, status,
            position_us, length_us, player_name
        Or None if no player / nothing playing.
        """
        try:
            player_bus_name = self._find_player()
            if not player_bus_name:
                return None

            bus = self._get_bus()
            proxy = bus.get_object(player_bus_name, self.MPRIS_PATH)
            props = dbus.Interface(proxy, self.PROPS_IFACE)

            metadata = props.Get(self.PLAYER_IFACE, 'Metadata')
            status = str(props.Get(self.PLAYER_IFACE, 'PlaybackStatus'))

            title = str(metadata.get('xesam:title', ''))
            artists = metadata.get('xesam:artist', [])
            artist = str(artists[0]) if artists else ''
            album = str(metadata.get('xesam:album', ''))
            length_us = int(metadata.get('mpris:length', 0))

            # Album art: mpris:artUrl can be file:// or https://
            art_url = str(metadata.get('mpris:artUrl', ''))

            # Position (may not be supported by all players)
            try:
                position_us = int(props.Get(self.PLAYER_IFACE, 'Position'))
            except dbus.exceptions.DBusException:
                position_us = 0

            short_name = player_bus_name[len(self.MPRIS_PREFIX):]
            # Strip instance suffixes like ".instance12345"
            short_name = short_name.split('.')[0]

            return {
                'title': title,
                'artist': artist,
                'album': album,
                'art_url': art_url,
                'status': status,
                'position_us': position_us,
                'length_us': length_us,
                'player_name': short_name,
            }

        except dbus.exceptions.DBusException:
            # Bus went away, player quit, etc.
            self._bus = None
            return None
        except Exception:
            return None


class AlbumArtCache:
    """Cache album art images, handling both file:// and http(s):// URLs."""

    def __init__(self):
        self._url = None
        self._image = None

    def get(self, art_url: str) -> Image.Image | None:
        """Get album art as a PIL Image, fetching/caching as needed.

        Returns None if the URL is empty or the fetch fails.
        """
        if not art_url:
            self._url = None
            self._image = None
            return None

        # Return cached image if URL hasn't changed
        if art_url == self._url and self._image is not None:
            return self._image

        # URL changed — fetch new art
        self._url = art_url
        self._image = None

        try:
            parsed = urlparse(art_url)
            if parsed.scheme == 'file':
                local_path = unquote(parsed.path)
                if os.path.exists(local_path):
                    self._image = Image.open(local_path).convert('RGB')
            elif parsed.scheme in ('http', 'https'):
                resp = urlopen(art_url, timeout=5)
                data = resp.read()
                self._image = Image.open(io.BytesIO(data)).convert('RGB')
        except Exception:
            self._image = None

        return self._image


def _truncate_text(draw: ImageDraw.ImageDraw, text: str, font, max_width: int) -> str:
    """Truncate text with ellipsis to fit within max_width pixels."""
    if not text:
        return text
    bbox = draw.textbbox((0, 0), text, font=font)
    if bbox[2] - bbox[0] <= max_width:
        return text
    while len(text) > 1:
        text = text[:-1]
        test = text.rstrip() + '…'
        bbox = draw.textbbox((0, 0), test, font=font)
        if bbox[2] - bbox[0] <= max_width:
            return test
    return '…'


def _format_time(us: int) -> str:
    """Format microseconds as m:ss."""
    secs = max(0, int(us / 1_000_000))
    return f"{secs // 60}:{secs % 60:02d}"


def _draw_music_note(draw: ImageDraw.ImageDraw, cx: int, cy: int, size: int):
    """Draw a simple music note icon as a placeholder for missing album art."""
    # Note head (filled ellipse)
    r = size // 5
    head_y = cy + size // 4
    draw.ellipse([cx - r - 4, head_y - r + 2, cx + r - 4, head_y + r + 2],
                 fill=(120, 120, 120))
    # Stem
    stem_x = cx + r - 5
    draw.line([stem_x, head_y - 2, stem_x, cy - size // 3], fill=(120, 120, 120), width=3)
    # Flag
    draw.line([stem_x, cy - size // 3, stem_x + size // 5, cy - size // 5],
              fill=(120, 120, 120), width=3)


def make_nowplaying_frame(info: dict | None, art_cache: AlbumArtCache = None) -> Image.Image:
    """Render the Now Playing display.

    Layout (240x240):
        - Album art centered at top (130x130)
        - Track title below art
        - Artist name below title
        - Progress bar near bottom
        - Time labels at very bottom
    """
    img = Image.new('RGB', (LCD_WIDTH, LCD_HEIGHT), (18, 18, 18))
    draw = ImageDraw.Draw(img)

    try:
        font_title = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 17)
        font_artist = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 14)
        font_time = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 11)
        font_status = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 13)
    except OSError:
        font_title = ImageFont.load_default()
        font_artist = font_title
        font_time = font_title
        font_status = font_title

    cx = LCD_WIDTH // 2

    # -- No player / nothing playing --
    if info is None or not info.get('title'):
        _draw_music_note(draw, cx, 100, 60)
        draw.text((cx, 155), "No music playing", fill=(100, 100, 100),
                  font=font_status, anchor="mt")
        return img

    # -- Album art --
    art_x = (LCD_WIDTH - ART_SIZE) // 2
    art_y = ART_MARGIN_TOP

    art_loaded = False
    art_url = info.get('art_url', '')
    if art_url and art_cache is not None:
        art_img = art_cache.get(art_url)
        if art_img is not None:
            try:
                art = art_img.resize((ART_SIZE, ART_SIZE), Image.LANCZOS)
                img.paste(art, (art_x, art_y))
                art_loaded = True
            except Exception:
                pass

    if not art_loaded:
        # Dark placeholder box with music note
        draw.rounded_rectangle([art_x, art_y, art_x + ART_SIZE, art_y + ART_SIZE],
                               radius=8, fill=(40, 40, 40))
        _draw_music_note(draw, cx, art_y + ART_SIZE // 2, 50)

    # -- Track title --
    text_max_w = LCD_WIDTH - 24
    title_y = art_y + ART_SIZE + 10
    title = _truncate_text(draw, info['title'], font_title, text_max_w)
    draw.text((cx, title_y), title, fill=(255, 255, 255), font=font_title, anchor="mt")

    # -- Artist --
    artist_y = title_y + 22
    artist = _truncate_text(draw, info['artist'], font_artist, text_max_w)
    draw.text((cx, artist_y), artist, fill=(170, 170, 170), font=font_artist, anchor="mt")

    # -- Progress bar --
    bar_y = 207
    bar_h = 5
    bar_margin = 20
    bar_left = bar_margin
    bar_right = LCD_WIDTH - bar_margin
    bar_width = bar_right - bar_left

    # Background track
    draw.rounded_rectangle([bar_left, bar_y, bar_right, bar_y + bar_h],
                           radius=bar_h // 2, fill=PROGRESS_BG)

    # Filled portion
    progress = 0.0
    if info['length_us'] > 0:
        progress = min(1.0, max(0.0, info['position_us'] / info['length_us']))

    if progress > 0.01:
        fill_right = bar_left + int(bar_width * progress)
        fill_right = max(fill_right, bar_left + bar_h)  # minimum visible
        draw.rounded_rectangle([bar_left, bar_y, fill_right, bar_y + bar_h],
                               radius=bar_h // 2, fill=SPOTIFY_GREEN)

        # Dot at current position
        dot_r = 5
        dot_cx = fill_right
        dot_cy = bar_y + bar_h // 2
        draw.ellipse([dot_cx - dot_r, dot_cy - dot_r, dot_cx + dot_r, dot_cy + dot_r],
                     fill=SPOTIFY_GREEN)

    # -- Time labels --
    time_y = bar_y + bar_h + 5
    pos_str = _format_time(info['position_us'])
    len_str = _format_time(info['length_us'])

    draw.text((bar_left, time_y), pos_str, fill=(170, 170, 170), font=font_time, anchor="lt")
    draw.text((bar_right, time_y), len_str, fill=(170, 170, 170), font=font_time, anchor="rt")

    # -- Pause indicator --
    if info['status'] == 'Paused':
        # Small pause icon next to time
        draw.text((cx, time_y), "⏸", fill=(170, 170, 170), font=font_time, anchor="mt")

    return img


# ==============================================================================
# CLI
# ==============================================================================

def main():
    parser = argparse.ArgumentParser(
        description='ID-Cooling FX360 LCD Controller for Linux',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s image photo.png          Display a static image
  %(prog)s color red                Display solid red
  %(prog)s color "#FF6600"          Display a hex color
  %(prog)s color 255,128,0          Display an RGB color
  %(prog)s gif animation.gif        Play a GIF animation
  %(prog)s sysinfo                  Show system stats
  %(prog)s nowplaying               Show current track (MPRIS)
  %(prog)s nowplaying --player spotify
  %(prog)s test                     Test connection (no image)
        """
    )

    sub = parser.add_subparsers(dest='command', required=True)

    # image command
    p_img = sub.add_parser('image', help='Display a static image')
    p_img.add_argument('path', help='Path to image file')
    p_img.add_argument('--quality', type=int, default=JPEG_QUALITY,
                       help=f'JPEG quality (default: {JPEG_QUALITY})')
    p_img.add_argument('--duration', type=float, default=None,
                       help='Display duration in seconds (default: until Ctrl+C)')

    # color command
    p_color = sub.add_parser('color', help='Display a solid color')
    p_color.add_argument('color', help='Color name, hex (#RRGGBB), or R,G,B')

    # gif command
    p_gif = sub.add_parser('gif', help='Play a GIF animation')
    p_gif.add_argument('path', help='Path to GIF file')
    p_gif.add_argument('--fps', type=float, default=None,
                       help='Override FPS (default: from GIF timing)')

    # sysinfo command
    p_sys = sub.add_parser('sysinfo', help='Display system information')
    p_sys.add_argument('--interval', type=float, default=1.0,
                       help='Update interval in seconds (default: 1.0)')

    # nowplaying command
    p_np = sub.add_parser('nowplaying', help='Show currently playing track (MPRIS)')
    p_np.add_argument('--player', type=str, default=None,
                      help='Target a specific player (e.g. spotify, firefox, vlc)')
    p_np.add_argument('--interval', type=float, default=1.0,
                      help='Update interval in seconds (default: 1.0)')

    # test command
    sub.add_parser('test', help='Test USB connection')

    args = parser.parse_args()

    # Connect to device
    lcd = IDCoolingLCD()
    if not lcd.connect():
        sys.exit(1)

    # Handle Ctrl+C gracefully
    def signal_handler(sig, frame):
        print("\nStopping...")
        lcd.stop()
    signal.signal(signal.SIGINT, signal_handler)

    try:
        if args.command == 'test':
            print("Connection successful! Sending keepalive...")
            lcd.send_keepalive()
            print("Device responded. Everything looks good!")

        elif args.command == 'color':
            print(f"Displaying solid color: {args.color}")
            img = make_solid_color(args.color)
            lcd.display_static(img)

        elif args.command == 'image':
            if not os.path.exists(args.path):
                print(f"ERROR: File not found: {args.path}")
                sys.exit(1)
            print(f"Displaying image: {args.path}")
            img = Image.open(args.path)
            lcd.display_static(img, duration=getattr(args, 'duration', None))

        elif args.command == 'gif':
            if not os.path.exists(args.path):
                print(f"ERROR: File not found: {args.path}")
                sys.exit(1)
            frames, gif_fps = load_gif_frames(args.path)
            fps = args.fps if args.fps else gif_fps
            print(f"Loaded {len(frames)} frames from {args.path}")
            lcd.display_animation(frames, fps=fps)

        elif args.command == 'sysinfo':
            print("Displaying system info (Ctrl+C to stop)...")
            while not lcd._stop_event.is_set():
                img = make_sysinfo_frame()
                jpeg = image_to_jpeg(img)

                # Send multiple frames per update for smooth display
                frame_end = time.monotonic() + args.interval
                while time.monotonic() < frame_end and not lcd._stop_event.is_set():
                    lcd.send_frame(jpeg)
                    if time.monotonic() - lcd._last_keepalive > KEEPALIVE_INTERVAL:
                        lcd.send_keepalive()
                    time.sleep(FRAME_INTERVAL)

        elif args.command == 'nowplaying':
            if not HAS_DBUS:
                print("ERROR: dbus-python is required for nowplaying mode.")
                print("Install with: pip install dbus-python")
                sys.exit(1)

            mpris = MPRISClient(player_name=args.player)
            art_cache = AlbumArtCache()
            target = args.player or "any player"
            print(f"Displaying now playing from {target} (Ctrl+C to stop)...")

            while not lcd._stop_event.is_set():
                info = mpris.get_now_playing()
                if info:
                    status_icon = "▶" if info['status'] == 'Playing' else "⏸"
                    print(f"\r  {status_icon} {info['artist']} — {info['title']}", end='', flush=True)

                img = make_nowplaying_frame(info, art_cache=art_cache)
                jpeg = image_to_jpeg(img)

                frame_end = time.monotonic() + args.interval
                while time.monotonic() < frame_end and not lcd._stop_event.is_set():
                    lcd.send_frame(jpeg)
                    if time.monotonic() - lcd._last_keepalive > KEEPALIVE_INTERVAL:
                        lcd.send_keepalive()
                    time.sleep(FRAME_INTERVAL)

    finally:
        lcd.disconnect()
        print("Disconnected.")


if __name__ == '__main__':
    main()
