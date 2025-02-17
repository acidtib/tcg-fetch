from datasets import load_dataset, Features, ClassLabel, Value, Image
import os
from PIL import Image as PILImage
import io
import json

def get_directory_size(path):
    """Get total size of directory, return 0 if directory doesn't exist"""
    if not os.path.exists(path):
        return 0
    total_size = 0
    for dirpath, dirnames, filenames in os.walk(path):
        for f in filenames:
            fp = os.path.join(dirpath, f)
            total_size += os.path.getsize(fp)
    return total_size

def count_examples(path):
    """Count total examples in directory, return 0 if directory doesn't exist"""
    if not os.path.exists(path):
        return 0
    return sum(len(os.listdir(os.path.join(path, d))) for d in os.listdir(path))

def embed_images(example):
    # Open image and convert to bytes
    img = PILImage.open(example["image"].filename)
    img_byte_arr = io.BytesIO()
    img.save(img_byte_arr, format=img.format or 'JPEG')
    img_byte_arr = img_byte_arr.getvalue()
    
    # Get just the image filename without the UUID folder
    full_path = example["image"].filename
    image_filename = os.path.basename(full_path)  # Get just the filename
    
    # Get the label (UUID folder name) and convert to numeric class
    label_uuid = os.path.basename(os.path.dirname(full_path))
    label_id = label_to_id[label_uuid]
    
    # Return new example with embedded image and numeric label
    return {
        "image": {
            "bytes": img_byte_arr,
            "path": image_filename
        },
        "label": label_id
    }

# Get unique labels first
data_dir = r"E:\magic-dataset\data"
train_dir = os.path.join(data_dir, "train")
val_dir = os.path.join(data_dir, "validation")
test_dir = os.path.join(data_dir, "test")

# Create output directory if it doesn't exist
git_dir = r"E:\turtle_code\tcg-magic"
output_dir = os.path.join(git_dir, "data")
os.makedirs(output_dir, exist_ok=True)

# Ensure train directory exists
if not os.path.exists(train_dir):
    raise FileNotFoundError(f"Training directory not found at {train_dir}")

labels = sorted(os.listdir(train_dir))
num_labels = len(labels)
print(f"Found {num_labels} unique labels: {labels[:5]}...")

# Create label mappings
label_to_id = {label: idx for idx, label in enumerate(labels)}
id_to_label = {idx: label for idx, label in enumerate(labels)}

# Calculate dataset sizes
train_size = get_directory_size(train_dir)
val_size = get_directory_size(val_dir)
test_size = get_directory_size(test_dir)
total_size = train_size + val_size + test_size

# Count examples in each split
train_examples = count_examples(train_dir)
val_examples = count_examples(val_dir)
test_examples = count_examples(test_dir)

print(f"\nDataset statistics:")
print(f"Train: {train_examples} examples, {train_size/1024/1024:.2f}MB")
print(f"Validation: {val_examples} examples, {val_size/1024/1024:.2f}MB")
print(f"Test: {test_examples} examples, {test_size/1024/1024:.2f}MB")
print(f"Total: {train_examples + val_examples + test_examples} examples, {total_size/1024/1024:.2f}MB")

# Prepare dataset info content
dataset_info_content = "dataset_info:\n"
dataset_info_content += "  features:\n"
dataset_info_content += "  - name: image\n    dtype: image\n"
dataset_info_content += "  - name: label\n    dtype:\n        class_label:\n          names:\n"

# Add first 50 label mappings as examples
for i, (idx, label) in enumerate(id_to_label.items()):
    if i < 50:  # Only show first 50 mappings
        dataset_info_content += f"            '{idx}': {label}\n"
    else:
        break

dataset_info_content += "            # ... additional labels truncated, see label_mapping.json for complete list\n"
dataset_info_content += "  splits:\n"

if train_examples > 0:
    dataset_info_content += f"  - name: train\n    num_bytes: {train_size}\n    num_examples: {train_examples}\n"
if val_examples > 0:
    dataset_info_content += f"  - name: validation\n    num_bytes: {val_size}\n    num_examples: {val_examples}\n"
if test_examples > 0:
    dataset_info_content += f"  - name: test\n    num_bytes: {test_size}\n    num_examples: {test_examples}\n"

dataset_info_content += f"  download_size: {total_size}\n"
dataset_info_content += f"  dataset_size: {total_size}\n"

# Save label mappings to a separate file
with open(os.path.join(git_dir, 'label_mapping.json'), 'w') as f:
    json.dump(id_to_label, f, indent=2)

# Read existing README content
readme_path = os.path.join(git_dir, "README.md")
if os.path.exists(readme_path):
    with open(readme_path, "r") as f:
        existing_content = f.read()
    
    # Find the dataset_info section
    start_marker = "---\ndataset_info:"
    end_marker = "\nconfigs:"
    
    if start_marker in existing_content and end_marker in existing_content:
        # Replace only the dataset_info section
        start_idx = existing_content.find(start_marker)
        end_idx = existing_content.find(end_marker)
        
        new_content = existing_content[:start_idx] + "---\n" + dataset_info_content + existing_content[end_idx:]
        
        with open(readme_path, "w") as f:
            f.write(new_content)
    else:
        # If no existing dataset_info section, create a new one
        with open(readme_path, "w") as f:
            f.write("---\n" + dataset_info_content + "\nconfigs:")
else:
    # If README doesn't exist, create it with just the dataset_info section
    with open(readme_path, "w") as f:
        f.write("---\n" + dataset_info_content + "\nconfigs:")

# Load the dataset with proper features
features = Features({
    "image": Image(),
    "label": ClassLabel(num_classes=num_labels, names=labels)
})

dataset = load_dataset(
    "imagefolder",
    data_dir=data_dir,
    features=features
)

print("Original dataset:", dataset)

# Embed images in each split
embedded_dataset = {}
for split_name, split_dataset in dataset.items():
    print(f"\nProcessing {split_name} split...")
    # Print some label statistics
    label_counts = {}
    for example in split_dataset:
        label = example["label"]
        label_name = id_to_label[label]
        label_counts[label_name] = label_counts.get(label_name, 0) + 1
    print(f"Label distribution in {split_name}: {dict(list(label_counts.items())[:5])}...")
    
    embedded_dataset[split_name] = split_dataset.map(embed_images)

# Convert each split to sharded parquet files
for split_name, split_dataset in embedded_dataset.items():
    # Calculate number of shards based on target shard size of 420MB
    target_shard_size = 420 * 1024 * 1024  # 420MB in bytes
    
    # Get a sample of images to estimate average size
    sample_size = min(100, len(split_dataset))
    sample_total = 0
    for example in split_dataset.select(range(sample_size)):
        img_byte_arr = io.BytesIO()
        example["image"].save(img_byte_arr, format=example["image"].format or 'JPEG')
        sample_total += len(img_byte_arr.getvalue())
    avg_image_size = sample_total / sample_size
    
    # Estimate total size and calculate number of shards
    estimated_total_size = avg_image_size * len(split_dataset)
    num_shards = max(1, int(estimated_total_size // target_shard_size + (1 if estimated_total_size % target_shard_size else 0)))
    
    print(f"\nSaving {split_name} split into {num_shards} shards...")
    print(f"Estimated total split size: {estimated_total_size / (1024 * 1024):.2f}MB")
    print(f"Average image size: {avg_image_size / 1024:.2f}KB")
    
    for index in range(num_shards):
        shard = split_dataset.shard(index=index, num_shards=num_shards, contiguous=True)
        # Use Hugging Face naming convention: split-XXXXX-of-YYYYY.parquet
        output_path = os.path.join(output_dir, f"{split_name}-{index:05d}-of-{num_shards:05d}.parquet")
        shard.to_parquet(output_path)
        print(f"Saved shard {index + 1}/{num_shards} to {output_path}")

print("\nDone! The parquet files now contain embedded images and numeric labels that can be uploaded to Hugging Face.")