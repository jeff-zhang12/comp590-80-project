#!/usr/bin/env python3
"""
ROI Compositor for differential compression.
Overlays high-quality ROI regions from annotated video onto compressed background.

Usage: python roi_compositor.py <annotated_video> <compressed_bg> <output>
"""

import sys
import cv2
from pathlib import Path

BBOX_FILE = "bounding_boxes.txt"

def load_bounding_boxes(filepath):
    """Load bounding boxes from file."""
    boxes = []
    with open(filepath, 'r') as f:
        for line in f:
            parts = line.strip().split(',')
            if len(parts) == 4:
                x1, y1, x2, y2 = map(int, parts)
                boxes.append((x1, y1, x2, y2))
    return boxes

def composite_roi(annotated_video, compressed_bg, output_video):
    """Composite high-quality ROI from annotated video onto compressed background."""
    
    print("    Loading bounding boxes...")
    bounding_boxes = load_bounding_boxes(BBOX_FILE)
    print(f"      ✓ Loaded {len(bounding_boxes)} boxes")
    
    # Open videos
    cap_annotated = cv2.VideoCapture(annotated_video)
    cap_bg = cv2.VideoCapture(compressed_bg)
    
    if not cap_annotated.isOpened() or not cap_bg.isOpened():
        print("      ✗ Error: Could not open video files")
        return False
    
    # Get video properties
    fps = cap_annotated.get(cv2.CAP_PROP_FPS)
    width = int(cap_annotated.get(cv2.CAP_PROP_FRAME_WIDTH))
    height = int(cap_annotated.get(cv2.CAP_PROP_FRAME_HEIGHT))
    total_frames = int(cap_annotated.get(cv2.CAP_PROP_FRAME_COUNT))
    
    print(f"      ✓ Video: {width}x{height} @ {fps}fps")
    
    # Create output writer
    fourcc = cv2.VideoWriter_fourcc(*'mp4v')
    out = cv2.VideoWriter(output_video, fourcc, fps, (width, height))
    
    frame_idx = 0
    
    print("      Processing frames...")
    while True:
        ret_annotated, frame_annotated = cap_annotated.read()
        ret_bg, frame_bg = cap_bg.read()
        
        if not ret_annotated or not ret_bg:
            break
        
        # Get bounding box for this frame
        if frame_idx < len(bounding_boxes):
            x1, y1, x2, y2 = bounding_boxes[frame_idx]
        else:
            x1, y1, x2, y2 = bounding_boxes[-1]
        
        # Ensure valid coordinates
        x1 = max(0, min(x1, width - 1))
        y1 = max(0, min(y1, height - 1))
        x2 = max(x1 + 1, min(x2, width))
        y2 = max(y1 + 1, min(y2, height))
        
        # Start with compressed background
        output_frame = frame_bg.copy()
        
        # Extract ROI from annotated video (with red box)
        roi_annotated = frame_annotated[y1:y2, x1:x2].copy()
        
        # Overlay high-quality ROI onto compressed background
        output_frame[y1:y2, x1:x2] = roi_annotated
        
        # Write frame
        out.write(output_frame)
        
        frame_idx += 1
        
        if frame_idx % 30 == 0:
            progress = 100 * frame_idx / total_frames
            print(f"        {frame_idx}/{total_frames} frames ({progress:.1f}%)")
    
    # Cleanup
    cap_annotated.release()
    cap_bg.release()
    out.release()
    
    print(f"      ✓ Composited {frame_idx} frames")
    return True

if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("Usage: python roi_compositor.py <annotated_video> <compressed_bg> <output>")
        sys.exit(1)
    
    annotated_video = sys.argv[1]
    compressed_bg = sys.argv[2]
    output_video = sys.argv[3]
    
    # Verify files exist
    if not Path(annotated_video).exists():
        print(f"Error: {annotated_video} not found")
        sys.exit(1)
    
    if not Path(compressed_bg).exists():
        print(f"Error: {compressed_bg} not found")
        sys.exit(1)
    
    if not Path(BBOX_FILE).exists():
        print(f"Error: {BBOX_FILE} not found")
        sys.exit(1)
    
    success = composite_roi(annotated_video, compressed_bg, output_video)
    sys.exit(0 if success else 1)

