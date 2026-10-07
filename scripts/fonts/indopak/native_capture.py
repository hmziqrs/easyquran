from PIL import Image


def stitch_native_capture(output, target, capture):
    width, height = capture["size"]
    if width < 1 or height < 1 or not capture["tiles"]:
        raise ValueError("Invalid native capture size or empty tiles")
    result = Image.new("RGB", (width, height), "white")
    covered = 0
    for tile in capture["tiles"]:
        with Image.open(output / tile["file"]) as source:
            if tuple(tile["pixels"]) != source.size:
                raise ValueError("Native tile dimensions differ from recorded PNG")
            left, top, right, bottom = tile["crop"]
            if not (0 <= left < right <= source.width and 0 <= top < bottom <= source.height):
                raise ValueError("Native tile crop leaves viewport image")
            if right - left != width:
                raise ValueError("Native tile width differs from specimen")
            destination = tile["destination"]
            end = destination + bottom - top
            if not (0 <= destination <= covered < end <= height):
                raise ValueError("Native tiles have missing, repeated or excess rows")
            result.paste(source.crop((left, top, right, bottom)).convert("RGB"), (0, destination))
            covered = end
    if covered != height:
        raise ValueError("Native tiles do not cover full specimen")
    result.save(output / target)
