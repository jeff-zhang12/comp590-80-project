import cv2

VIDEO_PATH = "videos/input.mp4"
OUTPUT_FILE = "bounding_boxes.txt"

box_width = 100
box_height = 100

mouse_x, mouse_y = 0, 0

def mouse_move(event, x, y, flags, param):
    global mouse_x, mouse_y
    if event == cv2.EVENT_MOUSEMOVE:
        mouse_x = x
        mouse_y = y

cap = cv2.VideoCapture(VIDEO_PATH)
frame_count = 0
video_width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
video_height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))

cv2.namedWindow("Frame")
cv2.setMouseCallback("Frame", mouse_move)

with open(OUTPUT_FILE, "w") as f:
    while True:
        ret, frame = cap.read()
        if not ret:
            break

        half_w = box_width // 2
        half_h = box_height // 2

        tl_x = max(0, min(mouse_x - half_w, video_width - 1))
        tl_y = max(0, min(mouse_y - half_h, video_height - 1))
        br_x = max(0, min(mouse_x + half_w, video_width - 1))
        br_y = max(0, min(mouse_y + half_h, video_height - 1))

        top_left = (tl_x, tl_y)
        bottom_right = (br_x, br_y)

        frame_copy = frame.copy()
        cv2.rectangle(frame_copy, top_left, bottom_right, (0, 255, 0), 2)
        cv2.putText(frame_copy, f"Frame {frame_count}", (10,30),
                    cv2.FONT_HERSHEY_SIMPLEX, 1, (0,0,255), 2)
        cv2.imshow("Frame", frame_copy)

        f.write(f"{top_left[0]},{top_left[1]},{bottom_right[0]},{bottom_right[1]}\n")
        frame_count += 1

        key = cv2.waitKey(30) & 0xFF
        if key == ord('q'):
            break
        elif key == ord('w'):
            box_height += 10
        elif key == ord('s'):
            box_height = max(10, box_height - 10)
        elif key == ord('d'):
            box_width += 10
        elif key == ord('a'):
            box_width = max(10, box_width - 10)

cap.release()
cv2.destroyAllWindows()
