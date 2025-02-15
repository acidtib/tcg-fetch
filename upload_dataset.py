from datasets import load_dataset, Features, Image, Value
import os

# Define the features of the dataset
features = Features({
  'image': Image(),
  'label': Value('string')
})

# Load the dataset with our features
dataset = load_dataset(
  "imagefolder",
  data_dir="D:\magic-dataset\data",
  features=features
)

def get_label(item):
  # Extract filename without extension
  filename = os.path.splitext(os.path.basename(item['image'].filename))[0]
  return {'label': filename}

# Add labels based on image filename
dataset = dataset.map(get_label)

print(dataset)
print(dataset["train"][0])

# Push the dataset to the Hub
dataset.push_to_hub("acidtib/tcg-magic")
