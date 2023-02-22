# MSP

---40---
14-12-14

## Конечные разности

- [Конечные разности](http://files.school-collection.edu.ru/dlrstore/e6376ea7-84b2-1aed-76ce-3061969eab84/1001537A.htm)
- []()

Градиенты я не вычисляю для каждой точки - в добавление к исходному массиву у
меня есть массивы с уже вычисленными градиентами по df/dx и df/dy для каждой
точки, которая лежит не на границе массива, формулы простые (центральная
разность)

median filter

т.е. надо взять все точки в окне фильтра, отсортировать их и проверить не равно
ли значение центральной точки максимальному в отсортированном массиве, median
filter делает то же самое, только берет значение из середины (медианы)
отсортированного массива.

df(x, y)/dx = ( f(x+1, y) - f(x-1, y) ) / 2h;
df(x, y)/dy = ( f(x, y+1) - f(x, y-1) ) / 2h;

Один из самых распространенных приемов визуализации ряда числовых данных —
псевдокривая (гр. pseudos, ложь), грубо соединяющая точки, соответствующие
данным этого ряда. Проблема в том, что достоверными являются только основные
точки псевдокривой, а промежуточные точки либо не соответствуют
действительности, либо бессмысленны.

В экспериментальной статистике ученый имеет дело с конечным эмпирическим рядом
данных. Для поиска закономерности, лежащей в основе экспериментальных данных,
применяется выравнивание (в англоязычной литературе fitting).

## Convolution / Convolve (сворачивание)

Convolution:

Filter:

- Kernel:
...
- Kernel:

Kernel:

- Padding
- Strides

- Docs:
  - [Типы ядер свертки](https://machinelearningmastery.ru/types-of-convolution-kernels-simplified-f040cb307c37/)
  - [Интуитивно понятное понимание сверток для глубокого обучения](https://machinelearningmastery.ru/intuitively-understanding-convolutions-for-deep-learning-1f6f42faee1/?source=post_page-----f040cb307c37----------------------)
  - [One Dimensional Convolutional Neural Networks](https://e2eml.school/convolution_one_d.html)
  - Video:
    - [Convolution in 1D and matrix-vector notation](https://www.youtube.com/watch?v=W2_nD85jL5s)
- Libs:
  - [arrayfire](https://arrayfire.org/arrayfire-rust/arrayfire/fn.convolve1.html)
  - [convolutions-rs](https://github.com/Conzel/convolutions-rs)
  - [fftconvolve](https://github.com/rhysnewell/fftconvolve)
  - [ndarray](https://github.com/rust-ndarray/ndarray/blob/master/examples/convo.rs)

## ML

- Docs:
  - [Сравнительный анализ алгоритмов машинного обучения](https://machinelearningmastery.ru/comparative-analysis-of-machine-learning-algorithms-888182847e84/)
  - [Сверточные нейронные сети](https://neerc.ifmo.ru/wiki/index.php?title=%D0%A1%D0%B2%D0%B5%D1%80%D1%82%D0%BE%D1%87%D0%BD%D1%8B%D0%B5_%D0%BD%D0%B5%D0%B9%D1%80%D0%BE%D0%BD%D0%BD%D1%8B%D0%B5_%D1%81%D0%B5%D1%82%D0%B8)

ML:

- Supervised - labeled input:
  - Regression - numerical or continuous output;
  - Classification - discrete or categorical output;
- Unsupervised - unlabeled input.

- `f[x]`: input function
- `h[x]`: (PSF) point spread function or (PRF) point response function
