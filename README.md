# Soustraire: REMI Imagesubtractor Implementation in Rust

Soustraire is a Rust implementation for REMI imagesubtractor, designed to process and analyze image differences efficiently. This application allows users to subtract previous images from current ones, calculate differences, and analyze the results. Below is a detailed guide on how to install, build, and run Soustraire on your system.

## Prerequisites
- **Rust**: Ensure that Rust is installed on your system. If not, you can install it from [here](https://www.rust-lang.org/ja/tools/install).
- **Naming**: Soustraire processes images based on lexicographical order. Ensure that your images are named in lexicographical order to maintain the correct subtraction sequence.


## Installation and Usage

1. **Clone the Repository:**
    ```shell
    git clone https://github.com/lycantrope/soustraire
    cd soustraire
    ```

2. **Build and Run the Application:**
    ```shell
    cargo run --release
    ```

3. **Optional: Build with SIMD Optimization (Intel AVX2/AVX-512)**
   - **MacOS/Linux:**
    ```bash
    RUSTFLAGS='-C target-feature=+avx' cargo build --release
    ```
   - **Windows:**
      1. Set `RUSTFLAGS` to activate SIMD features.
        ```powershell
        set RUSTFLAGS -C target-feature=+avx
        ```
      2. Build
        ```powershell
        cargo build --release
        ```

## Algorithm Overview
1. **Delta Calculation:**
    - Compute the difference between the previous image and the current image.

2. **Mean and Standard Deviation:**
    - Calculate the mean() and standard deviation(std) from the delta.

3. **Normalization:**
    - Normalize the image, ranging from -10.0×std to +10.0×std.

4. **Binarization:**
    - Binarize the image using a threshold (n times std below mean, e.g., 2.5×std represents 2.5×std below the mean value).

5. **Pixel Count:**
    - Count the number of pixels where the value was 0 (representing the different parts between current and previous images).

## Output Format
The application generates two output files:

### 1. `Area.csv`
- **Header**: Contains columns for "Area".
- **Data**: Each row represents the subtracted time-point of pixel counts from each ROI.

### 2. `Roi.json`
- **JSON Structure**:
    ```json
    {
      "nrow": 6,
      "ncol": 8,
      "x": 13,
      "y": 8,
      "xinterval": 129,
      "yinterval": 130,
      "width": 78,
      "height": 78,
      "rotate": -0.1,
      "rois": [
        {
          "x": 13,
          "y": 8,
          "width": 78,
          "height": 78,
          "index": 0
        }
      ]
    }
    ```
    - `"nrow"`: Number of rows in the image grid.
    - `"ncol"`: Number of columns in the image grid.
    - `"x"`: X-coordinate of the top-left corner of the grid.
    - `"y"`: Y-coordinate of the top-left corner of the grid.
    - `"xinterval"`: Horizontal interval between grid cells.
    - `"yinterval"`: Vertical interval between grid cells.
    - `"width"`: Width of each grid cell.
    - `"height"`: Height of each grid cell.
    - `"rotate"`: Rotation angle of the grid.
    - `"rois"`: Array of regions of interest (ROIs) within the grid.
        - `"x"`: X-coordinate of the top-left corner of the ROI.
        - `"y"`: Y-coordinate of the top-left corner of the ROI.
        - `"width"`: Width of the ROI.
        - `"height"`: Height of the ROI.
        - `"index"`: Index of the ROI.

## Important Notes
- **WASM Not Supported:**
    - Please note that this application does not support WebAssembly (WASM).
- **Image Naming Convention:**
    - The images must be sorted by lexicographical order. To maintain the correct subtraction order, ensure that the images are named in lexicographical order.



## License
This project is licensed under the [MIT License](LICENSE).

Feel free to explore, modify, and utilize the code for your own purposes. If you have any questions or issues, please create a new [issue](https://github.com/lycantrope/soustraire/issues) on the GitHub repository. Happy coding!
