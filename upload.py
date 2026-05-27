import struct
import serial

PORT='/dev/ttyUSB0'
BAUD=115200

with open('http_rust.wasm','rb') as f:
    data=f.read()

ser=serial.Serial(PORT,BAUD)

ser.write(struct.pack("<I",len(data)))

ser.write(data)

print("UPLOAD DONE")

ser.close()