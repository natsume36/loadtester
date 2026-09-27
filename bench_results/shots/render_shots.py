# -*- coding: utf-8 -*-
"""把真实捕获的控制台输出渲染成终端风格截图 PNG（Windows Terminal 风格）。"""
from PIL import Image, ImageDraw, ImageFont
import os

HERE = os.path.dirname(os.path.abspath(__file__))
FONT_PATH = "C:/Windows/Fonts/msyh.ttc"
MONO_PATH = "C:/Windows/Fonts/consola.ttf"

BG = (12, 12, 12)
BAR = (32, 32, 32)
FG = (204, 204, 204)
PROMPT_FG = (120, 200, 120)
DIM = (140, 140, 140)
ACCENT = (240, 240, 240)

def render_lines(lines, png_file, title, command, width=1080, font_size=16, pad=18):
    font = ImageFont.truetype(FONT_PATH, font_size)
    bar_font = ImageFont.truetype(FONT_PATH, 13)
    line_h = int(font_size * 1.65)
    bar_h = 34
    prompt_lines = ["PS C:\\Users\\Natsume\\Desktop\\rust> " + command]
    total_lines = prompt_lines + lines
    height = bar_h + pad * 2 + line_h * len(total_lines)
    img = Image.new("RGB", (width, height), BG)
    d = ImageDraw.Draw(img)
    d.rectangle([0, 0, width, bar_h], fill=BAR)
    for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
        d.ellipse([12 + i * 22, bar_h // 2 - 5, 22 + i * 22, bar_h // 2 + 5], fill=c)
    d.text((width // 2 - len(title) * 4, 8), title, font=bar_font, fill=(180, 180, 180))
    y = bar_h + pad
    for pl in prompt_lines:
        head, cmd = pl.split("> ", 1)
        d.text((pad, y), head + "> ", font=font, fill=PROMPT_FG)
        w = d.textlength(head + "> ", font=font)
        d.text((pad + w, y), cmd, font=font, fill=ACCENT)
        y += line_h
    for l in lines:
        color = FG
        if l.startswith("-----"):
            color = (97, 174, 239)
        elif "QPS" in l or "延迟" in l:
            color = (220, 220, 170)
        elif "test result" in l:
            color = (134, 239, 172)
        d.text((pad, y), l, font=font, fill=color)
        y += line_h
    img.save(os.path.join(HERE, png_file))
    print(png_file, img.size)

def render(txt_file, png_file, title, command, width=1080, font_size=16, pad=18, max_lines=None):
    with open(os.path.join(HERE, txt_file), encoding="utf-8", errors="replace") as f:
        lines = [l.rstrip("\n") for l in f.readlines()]
    lines = [l for l in lines if l.strip() != ""]
    if max_lines:
        lines = lines[:max_lines]
    render_lines(lines, png_file, title, command, width, font_size, pad)

render("help.txt", "shot_help.png", "loadtester --help", ".\\target\\release\\loadtester.exe --help", max_lines=26)
render("t1_console.txt", "shot_t1.png", "满负荷吞吐 20000 请求 x 50 并发", ".\\target\\release\\loadtester.exe http://127.0.0.1:8088/ping -n 20000 -C 50")
render("t2_console.txt", "shot_t2.png", "精准限流 500 QPS 持续 5 秒", ".\\target\\release\\loadtester.exe http://127.0.0.1:8088/ping --duration 5 --qps 500 -C 20")
render("t3_console.txt", "shot_t3.png", "慢接口延迟分布 200 请求 x 20 并发", ".\\target\\release\\loadtester.exe http://127.0.0.1:8088/slow -n 200 -C 20")
# cargo test：开头几行 + 分位数相关测试行 + 结果行
with open(os.path.join(HERE, "cargo_test.txt"), encoding="utf-8", errors="replace") as f:
    ct = [l.rstrip("\n").strip() for l in f if l.strip()]
head = [l for l in ct if "Finished" in l or "Running unittests" in l or l == "running 25 tests"]
sel = [l for l in ct if l.startswith("test ")][:5]
result = [l for l in ct if l.startswith("test result") and "25 passed" in l]
render_lines(head + sel + result, "shot_test.png", "cargo test 25/25", "cargo test")

# HTML 对比报告：直接嵌入 HTML 源文件方式不便，跳过；对比数据以表格呈现
