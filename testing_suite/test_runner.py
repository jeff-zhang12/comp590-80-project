import os
import sys
import random
import subprocess
import time
import cv2

# Configuration
INPUT_DIR = "testing_suite/input"
OUTPUT_DIR = "testing_suite/output"
RUST_BINARY_CMD = ["cargo", "run", "--release", "--"]

def ensure_dir(path):
    if not os.path.exists(path):
        os.makedirs(path)

def get_video_info(video_path):
    cap = cv2.VideoCapture(video_path)
    if not cap.isOpened():
        raise ValueError(f"Could not open video: {video_path}")
    
    width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
    height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
    frame_count = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    fps = cap.get(cv2.CAP_PROP_FPS)
    
    cap.release()
    return width, height, frame_count, fps

def generate_random_bboxes(width, height, frame_count, output_file):
    # Requirements: 25% of frame area
    # Let's make it a square or rectangle with area = 0.25 * width * height
    target_area = 0.25 * width * height
    # Aspect ratio 1:1 for simplicity? Or match video aspect ratio?
    # Let's do sqrt(area) for width and height
    box_w = int((target_area * (width/height))**0.5)
    box_h = int((target_area * (height/width))**0.5)
    
    # Actually simpler: box_w = width / 2, box_h = height / 2 => area = 1/4 * width * height
    box_w = width // 2
    box_h = height // 2
    
    # "Randomly moving". Let's do a simple random walk or just random positions.
    # To make it "moving", let's use a random walk to be smoother, or just random per frame?
    # "generate randomly moving bounding boxes" implies continuity usually, but random positions is also "randomly moving".
    # Let's implement a bounded random walk.
    
    x = random.randint(0, width - box_w)
    y = random.randint(0, height - box_h)
    
    with open(output_file, 'w') as f:
        for _ in range(frame_count):
            # Random step
            dx = random.randint(-10, 10)
            dy = random.randint(-10, 10)
            
            x = max(0, min(width - box_w, x + dx))
            y = max(0, min(height - box_h, y + dy))
            
            x2 = x + box_w
            y2 = y + box_h
            
            f.write(f"{x},{y},{x2},{y2}\n")

import csv

def run_test():
    ensure_dir(OUTPUT_DIR)
    
    input_files = [f for f in os.listdir(INPUT_DIR) if f.endswith(('.mp4', '.y4m', '.mkv', '.avi'))]
    
    if not input_files:
        print(f"No video files found in {INPUT_DIR}")
        return

    csv_output_path = os.path.join(OUTPUT_DIR, "results.csv")
    csv_headers = [
        "Video", "Input Size (MB)", "Duration (s)", 
        "Dynamic Output (MB)", "Dynamic Time (s)", "Dynamic FPS", "Dynamic Bitrate (kbps)", "Dynamic Ratio",
        "Baseline Output (MB)", "Baseline Time (s)", "Baseline FPS", "Baseline Bitrate (kbps)", "Baseline Ratio",
        "Space Saving (%)"
    ]

    results = []

    print(f"{'Video':<20} | {'In(MB)':<7} | {'Dyn(MB)':<7} | {'Time(s)':<7} | {'FPS':<5} | {'Base(MB)':<8} | {'Time(s)':<7} | {'FPS':<5} | {'Save%':<6}")
    print("-" * 110)

    for filename in input_files:
        input_path = os.path.join(INPUT_DIR, filename)
        output_filename = os.path.splitext(filename)[0] + "_processed.mp4"
        output_path = os.path.join(OUTPUT_DIR, output_filename)
        bbox_path = "temp_bboxes.txt"
        
        try:
            width, height, frame_count, fps = get_video_info(input_path)
            video_duration = frame_count / fps if fps > 0 else 0
            
            # Generate bounding boxes
            generate_random_bboxes(width, height, frame_count, bbox_path)
            
            # Run Rust compressor (Dynamic)
            start_time = time.time()
            cmd_dynamic = RUST_BINARY_CMD + [input_path, output_path, bbox_path]
            result_dynamic = subprocess.run(cmd_dynamic, capture_output=True, text=True)
            dynamic_time = time.time() - start_time
            
            if result_dynamic.returncode != 0:
                print(f"Error processing {filename} (Dynamic):")
                print(result_dynamic.stderr)
                continue
            
            # Run Rust compressor (Baseline)
            # Average of 23 (ROI) and 51 (BG) is 37
            baseline_crf = "23" 
            output_baseline = os.path.join(OUTPUT_DIR, os.path.splitext(filename)[0] + "_baseline.mp4")
            
            start_time = time.time()
            cmd_baseline = RUST_BINARY_CMD + ["baseline", input_path, output_baseline, baseline_crf]
            result_baseline = subprocess.run(cmd_baseline, capture_output=True, text=True)
            baseline_time = time.time() - start_time
            
            if result_baseline.returncode != 0:
                print(f"Error processing {filename} (Baseline):")
                print(result_baseline.stderr) 
                
            input_size = os.path.getsize(input_path)
            output_size_dynamic = os.path.getsize(output_path) if os.path.exists(output_path) else 0
            output_size_baseline = os.path.getsize(output_baseline) if os.path.exists(output_baseline) else 0
            
            # Metrics Calculation
            dynamic_fps = frame_count / dynamic_time if dynamic_time > 0 else 0
            baseline_fps = frame_count / baseline_time if baseline_time > 0 else 0
            
            dynamic_bitrate = (output_size_dynamic * 8) / (video_duration * 1000) if video_duration > 0 else 0
            baseline_bitrate = (output_size_baseline * 8) / (video_duration * 1000) if video_duration > 0 else 0
            
            ratio_dynamic = output_size_dynamic / input_size if input_size > 0 else 0
            ratio_baseline = output_size_baseline / input_size if input_size > 0 else 0
            
            space_saving = ((output_size_baseline - output_size_dynamic) / output_size_baseline * 100) if output_size_baseline > 0 else 0
            
            # Store results
            row = [
                filename,
                f"{input_size/1024/1024:.2f}",
                f"{video_duration:.2f}",
                f"{output_size_dynamic/1024/1024:.2f}",
                f"{dynamic_time:.2f}",
                f"{dynamic_fps:.2f}",
                f"{dynamic_bitrate:.0f}",
                f"{ratio_dynamic:.3f}",
                f"{output_size_baseline/1024/1024:.2f}",
                f"{baseline_time:.2f}",
                f"{baseline_fps:.2f}",
                f"{baseline_bitrate:.0f}",
                f"{ratio_baseline:.3f}",
                f"{space_saving:.2f}"
            ]
            results.append(row)
            
            print(f"{filename[:20]:<20} | {input_size/1024/1024:>7.2f}M | {output_size_dynamic/1024/1024:>7.2f}M | {dynamic_time:>7.2f} | {dynamic_fps:>5.1f} | {output_size_baseline/1024/1024:>8.2f}M | {baseline_time:>7.2f} | {baseline_fps:>5.1f} | {space_saving:>6.2f}%")
            
        except Exception as e:
            print(f"Failed to process {filename}: {e}")
            import traceback
            traceback.print_exc()
        finally:
            if os.path.exists(bbox_path):
                os.remove(bbox_path)
    
    # Save to CSV
    try:
        with open(csv_output_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(csv_headers)
            writer.writerows(results)
        print(f"\nResults saved to {csv_output_path}")
    except IOError as e:
        print(f"Error saving CSV: {e}")

if __name__ == "__main__":
    run_test()
