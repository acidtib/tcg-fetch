from datasets import load_dataset
import os

# Load the dataset
dataset = load_dataset(
  "imagefolder",
  data_dir="E:\magic-dataset\data",
)

print(dataset)
print(dataset["train"][0])
print(dataset["train"][0]["image"].filename)

# Push the dataset to the Hub
dataset.push_to_hub("acidtib/tcg-magic")
