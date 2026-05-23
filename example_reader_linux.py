import serial
ser = serial.Serial("/dev/ttyACM1", 115200, timeout=1)
while True:
    b = ser.read(1)
    if b:
        print(f"{b[0]:02X}")
