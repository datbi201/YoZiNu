#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Test Zing MP3: search -> streaming -> recommend (auto-play similar songs).
Self-contained (Python standard library only). Run on the machine whose
network you want to test (e.g. a PC in Vietnam):

    python test_zing_stream_autoplay_v1.2.py ["search query"]

Set YOZINU_ZING_API_KEY and YOZINU_ZING_SECRET_KEY in the local environment
before running the test. The signing scheme follows the existing Zing adapter.
"""
import gzip, hashlib, hmac, io, json, os, sys, time
import urllib.request, urllib.parse, http.cookiejar

BASE = "https://zingmp3.vn"
VERSION = "1.6.34"
API_KEY = os.environ["YOZINU_ZING_API_KEY"]
SECRET_KEY = os.environ["YOZINU_ZING_SECRET_KEY"]
UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36"

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))
opener.addheaders = [("User-Agent", UA)]


def sha256_hex(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()


def sig_for(path: str, hash_input: str) -> str:
    return hmac.new(SECRET_KEY.encode(), (path + sha256_hex(hash_input)).encode(),
                    hashlib.sha512).hexdigest()


def call(path: str, params: dict, hash_input: str) -> dict:
    ctime = str(int(time.time()))
    q = dict(params)
    q.update({"ctime": ctime, "version": VERSION, "apiKey": API_KEY,
              "sig": sig_for(path, hash_input.replace("{ctime}", ctime))})
    url = BASE + path + "?" + urllib.parse.urlencode(q)
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Referer": BASE + "/"})
    with opener.open(req, timeout=25) as r:
        raw = r.read()
        if r.headers.get("Content-Encoding") == "gzip" or raw[:2] == b"\x1f\x8b":
            raw = gzip.decompress(raw)
        text = raw.decode("utf-8", "replace")
        try:
            return json.loads(text)
        except Exception:
            print("   [debug] non-JSON head:", text[:200])
            raise


def main() -> int:
    query = sys.argv[1] if len(sys.argv) > 1 else "Nơi Này Có Anh"
    print("== 0. Lay cookie tu trang chu ==")
    try:
        with opener.open(BASE + "/", timeout=25) as r:
            r.read()
        print("   OK, cookies:", [c.name for c in cj])
    except Exception as e:
        print("   LOI:", repr(e))
        return 1

    print(f"== 1. Tim kiem: {query!r} ==")
    res = call("/api/v2/search/multi", {"q": query}, "ctime={ctime}version=" + VERSION)
    print("   err:", res.get("err"), res.get("msg", ""))
    songs = (((res.get("data") or {}).get("songs")) or [])
    if not songs:
        print("   Khong co bai hat nao. Dung lai.")
        return 1
    for s in songs[:5]:
        print(f"   - {s.get('encodeId')} | {s.get('title')} | {s.get('artistsNames')} | {s.get('duration')}s")
    sid = songs[0].get("encodeId")

    print(f"== 2. Streaming cho bai dau tien ({sid}) ==")
    res = call("/api/v2/song/get/streaming", {"id": sid},
               "ctime={ctime}id=" + sid + "version=" + VERSION)
    print("   err:", res.get("err"), res.get("msg", ""))
    data = res.get("data") or {}
    print("   cac muc chat luong tra ve:", sorted(data.keys()) if isinstance(data, dict) else data)
    stream_url = None
    if isinstance(data, dict):
        for k in ("128", "320", "lossless"):
            if data.get(k):
                stream_url = data[k]
                print(f"   URL {k}: {stream_url[:110]}...")
                break
    stream_ok = False
    if stream_url:
        print("== 3. Tai thu 64 KB dau cua stream ==")
        try:
            req = urllib.request.Request(stream_url,
                                         headers={"User-Agent": UA, "Range": "bytes=0-65535"})
            with urllib.request.urlopen(req, timeout=25) as r:
                chunk = r.read()
            print(f"   HTTP OK, doc duoc {len(chunk)} byte, content-type: {r.headers.get('Content-Type')}")
            stream_ok = len(chunk) > 1000
        except Exception as e:
            print("   LOI tai stream:", repr(e))

    print(f"== 4. Chuoi bai tuong tu (autoplay) cho {sid} ==")
    # Verified chain (same as the official desktop app's SongService):
    # song/get/section-bottom -> playlists -> page/get/playlist -> songs
    rec_ok = False
    try:
        res = call("/api/v2/song/get/section-bottom", {"id": sid},
                   "ctime={ctime}id=" + sid + "version=" + VERSION)
        print(f"   section-bottom: err={res.get('err')} msg={res.get('msg','')}")
        pid = None
        for sec in (res.get("data") or []):
            items = sec.get("items") or []
            if items:
                pid = items[0].get("encodeId")
                print(f"   section {sec.get('sectionType')!r} ({sec.get('title')}) -> playlist {pid}")
                break
        if pid:
            res = call("/api/v2/page/get/playlist", {"id": pid},
                       "ctime={ctime}id=" + pid + "version=" + VERSION)
            d = res.get("data") or {}
            items = ((d.get("song") or {}).get("items")) or []
            print(f"   playlist {d.get('title')!r}: err={res.get('err')}, {len(items)} bai")
            for it in items[:5]:
                print(f"      - {it.get('encodeId')} | {it.get('title')} | {it.get('artistsNames')}")
            rec_ok = len(items) >= 2
    except Exception as e:
        print("   LOI:", repr(e))

    print("\n== KET LUAN ==")
    print("   STREAM:", "OK - phat duoc nhac" if stream_ok else
          ("co URL nhung tai that bai" if stream_url else "KHONG lay duoc URL (bi chan theo vung hoac can dang nhap/VIP)"))
    print("   RECOMMEND (tu dong chuyen bai tuong tu):", "OK" if rec_ok else "CHUA duoc - can tim endpoint khac")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
