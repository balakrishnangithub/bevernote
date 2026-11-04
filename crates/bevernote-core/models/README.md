# Bundled OCR models

`text-detection.onnx` and `text-recognition.onnx` are the official pre-trained Ocrs models by Robert Knight and Ocrs contributors, distributed under **Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)**.

- Project: https://github.com/robertknight/ocrs
- Model card and license: https://huggingface.co/robertknight/ocrs
- Detection source: https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx
- Recognition source: https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx
- Training data: HierText and synthetic data; see the model card and `ocrs-models` project.
- License: https://creativecommons.org/licenses/by-sa/4.0/

The files are bundled under their original filenames' corresponding model roles, without modifications to the model data. This license applies to the model files, not to Bevernote's GPL-licensed source code. If you adapt and share these models, retain attribution and distribute the adaptations under CC BY-SA 4.0.

Ocrs is an early-preview engine and currently recognizes the Latin alphabet. Recognition quality varies, especially for handwriting and non-Latin scripts.
