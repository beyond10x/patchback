model PatchRegressor {
    param w1: Tensor<f32, [32, 64]> = xavier_uniform();
    param w2: Tensor<f32, [64, 8]> = xavier_uniform();

    fn forward(x: Tensor<f32, [16, 32]>) -> Tensor<f32, [16, 8]> {
        let h = relu(matmul(x, w1));
        return matmul(h, w2);
    }
}

train FitPatches {
    model = PatchRegressor;
    input x: Tensor<f32, [16, 32]>;
    target y: Tensor<f32, [16, 8]>;
    loss = mse(model.forward(x), y);
    optimizer = sgd(lr = 0.01);
    steps = 5000;
    seed = 42;
    device = cpu;
}
