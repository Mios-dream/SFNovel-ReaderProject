const path = require('path');

/** @type {import('webpack').Configuration} */
module.exports = {
    module: {
        rules: [
            {
                test: /\.ts$/,
                use: ["ts-loader"],
                // exclude: /node_modules/,
            },
        ],
    },
    target: ['node', 'es6'],
    mode: "production",
    entry: {
        index: './src/main.ts',
    },
    output: {
        filename: '[name].js',
        path: path.resolve(__dirname, 'dist'),
    },
    resolve: {
        extensions: ['.js', '.ts'],
    },
};