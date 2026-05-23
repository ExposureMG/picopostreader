import serial
ser = serial.Serial("COM5", 115200, timeout=1)  # baud ignored for USB CDC but required by API
while True:
    b = ser.read(1)
    if b:
        print(f"{b[0]:02X}")