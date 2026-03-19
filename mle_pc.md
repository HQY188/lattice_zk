\subsection{Lattice-based Univariate Polynomial Commitment Scheme}

The lattice-based univariate polynomial commitment scheme is defined as follows:

\begin{itemize}
\item $\mathsf{PC.Setup}(1^\lambda, N) \to \mathsf{ck}$: Given security parameter $\lambda$ and polynomial degree bound $N = mn$, generate commitment key $\mathsf{ck} = (\mathbf{A}_0, \mathbf{A}_1)$, where $n = d\ell/r$, $\mathbf{A}_0 \leftarrow \mathcal{U}(R_q^{\mu \times \ell})$, and $\mathbf{A}_1 = [\mathbf{A}_1' \mid \mathbf{I}_{\mu}] \in R_q^{\mu \times (\mu+\nu)}$, where $\mathbf{A}_1' \leftarrow \mathcal{U}(R_q^{\mu \times \nu})$.

\item $\mathsf{PC.Com}(\mathsf{ck}, h(X)) \to (\vec{\mathfrak{h}}, \delta)$: Given polynomial $h(X) = \sum_{i=0}^{N-1} h_i X^i \in \mathbb{Z}_p[X]$, generate commitment $\vec{\mathfrak{h}}$ and opening information $\delta = (\vec{\bm{h}}, \vec{\bm{\eta}})$, where $\vec{\mathfrak{h}} = \vec{\mathfrak{h}}_0 \parallel \cdots \parallel \vec{\mathfrak{h}}_{m+1}$, $\vec{\bm{h}} = \vec{\bm{h}}_0 \parallel \cdots \parallel \vec{\bm{h}}_{m+1}$, $\vec{\bm{\eta}} = \vec{\bm{\eta}}_0 \parallel \cdots \parallel \vec{\bm{\eta}}_{m+1}$:

\begin{enumerate}
\item Define $\vec{h}_i = (h_{ni}, \ldots, h_{n(i+1)-1})$, where $0 \leq i < m$.
\item Sample $b_1, \ldots, b_{n-1} \leftarrow \mathcal{U}(\mathbb{Z}_p)$, define $\vec{h}_m = (b_1, \ldots, b_{n-1}, 0)$ and $\vec{h}_{m+1} = (0, -b_1, \ldots, -b_{n-1})$.
\item For $0 \leq i \leq m$, compute $\vec{\bm{h}}_i \leftarrow \mathsf{R.Ecd}(\vec{h}_i; \mathfrak{s}_1)$, $\vec{\bm{\eta}}_i \leftarrow \mathcal{D}_{\mathbb{Z}^d,\sigma_1}^{\mu+\nu}$, and $\vec{\mathfrak{h}}_i = \mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i \pmod{q}$.
\item Compute $\vec{\bm{h}}_{m+1} \leftarrow \mathsf{R.Ecd}(\vec{h}_{m+1}, \sqrt{m+2} \cdot \mathfrak{s}_3)$, $\vec{\bm{\eta}}_{m+1} \leftarrow \mathcal{D}_{\mathbb{Z}^d,\sqrt{m+2} \cdot \sigma_3}^{\mu+\nu}$, and $\vec{\mathfrak{h}}_{m+1} = \mathbf{A}_0 \vec{\bm{h}}_{m+1} + \mathbf{A}_1 \vec{\bm{\eta}}_{m+1} \pmod{q}$.
\end{enumerate}

\item $\mathsf{PC.Open}(\mathsf{ck}, \vec{\mathfrak{h}}, h(X), \delta) \to b$: Given commitment $\vec{\mathfrak{h}}$, opening information $\delta = (\vec{\bm{h}}, \vec{\bm{\eta}})$ and polynomial $h(X)$ of degree $< N$, output $1$ if the following conditions hold; otherwise output $0$:

\begin{enumerate}
\item $\| (\vec{2\bm{h}}_i||2 \vec{\bm{\eta}}_i) \|_2 \leq 2d\beta_{\mathsf{PC}.Open}$ for $0 \leq i \leq m$.
\item $\| (\vec{2\bm{h}}_{m+1}||2 \vec{\bm{\eta}}_{m+1}) \|_2 \leq 2\beta_{\mathsf{PC}}$, where $\beta_{\mathsf{PC}} = \beta_{\mathsf{PC}.Eval} + \frac{(b+1)(m+1)dr}{2} \cdot \beta_{\mathsf{PC}.Open}$.
\item $\vec{\mathfrak{h}}_i = \mathbf{A}_0 \vec{\bm{h}}_i + \mathbf{A}_1 \vec{\bm{\eta}}_i \pmod{q}$ for $0 \leq i \leq m+1$.
\item $h(X) = \sum_{i=0}^{m-1} X^{ni} \cdot \langle \vec{h}_i, \vec{X} \rangle + X \cdot \langle \vec{h}_m, \vec{X} \rangle + \langle \vec{h}_{m+1}, \vec{X} \rangle \pmod{p}$, where $\vec{X} = (1, X, \cdots, X^{n-1})$, and $\vec{h}_i = \frac{p+1}{2} \cdot \mathsf{Dcd}(2\vec{\bm{h}}_i) \pmod{p}$ for $0 \leq i \leq m+1$.
\end{enumerate}

\item $\mathsf{PC.Eval}(x, \delta) \to (y, \rho)$: Given opening information $\delta = (\vec{\bm{h}}, \vec{\bm{\eta}})$ and evaluation point $x \in \mathbb{Z}_p$, generate evaluation result $y$ and evaluation proof $\rho$:

\begin{enumerate}
\item Compute $\vec{\bm{e}} = \sum_{i=0}^{m-1} \mathsf{R.Ecd}(x^{ni}) \cdot \vec{\bm{h}}_i + \mathsf{R.Ecd}(x) \cdot \vec{\bm{h}}_m + \vec{\bm{h}}_{m+1}$ and $\vec{\bm{\varepsilon}} = \sum_{i=0}^{m-1} \mathsf{R.Ecd}(x^{ni}) \cdot \vec{\bm{\eta}}_i + \mathsf{R.Ecd}(x) \cdot \vec{\bm{\eta}}_m + \vec{\bm{\eta}}_{m+1}$.
\item Return evaluation proof $\rho = (\vec{\bm{e}}, \vec{\bm{\varepsilon}})$ and evaluation result $y = \langle \mathsf{Dcd}(\vec{\bm{e}}), (1, x, \cdots, x^{n-1}) \rangle \pmod{p}$.
\end{enumerate}

\item $\mathsf{PC.Verify}(\mathsf{ck}, \vec{\mathfrak{h}}, x, y, \rho) \to b$: Given commitment $\vec{\mathfrak{h}}$, evaluation point $x$, evaluation result $y$ and proof $\rho = (\vec{\bm{e}}, \vec{\bm{\varepsilon}})$, output $1$ if the following conditions hold; otherwise output $0$:

\begin{enumerate}
\item $\| (\vec{\bm{e}}, \vec{\bm{\varepsilon}}) \|_2 \leq \beta_{\mathsf{PC}.Eval}$.
\item $y = \langle \mathsf{Dcd}(\vec{\bm{e}}), (1, x, \cdots, x^{n-1}) \rangle \pmod{p}$.
\item $\mathbf{A}_0 \vec{\bm{e}} + \mathbf{A}_1 \vec{\bm{\varepsilon}} = \sum_{i=0}^{m-1} \mathsf{R.Ecd}(x^{ni}) \cdot \vec{\mathfrak{h}}_i + \mathsf{R.Ecd}(x) \cdot \vec{\mathfrak{h}}_m + \vec{\mathfrak{h}}_{m+1} \pmod{q}$.
\end{enumerate}
\end{itemize}
\subsection{Lattice-based Multilinear Polynomial Commitment Scheme}

The lattice-based multilinear polynomial commitment scheme is defined as follows:

\begin{itemize}
\item \textbf{PC.Setup($1^\lambda, l$)} $\to$ ck: Given security parameter $\lambda$ and Boolean input length $l$, select integer $\iota \geq 2$ such that $l/\iota$ is an integer. Call the Setup algorithm of univariate polynomial commitment:
\[
\text{ck}_{\text{uni}} \gets \text{PC.Setup}(1^\lambda, N = 2^{l - l/\iota})
\]
Output $\text{ck} = (\text{ck}_{\text{uni}}, l, \iota)$.

\item \textbf{PC.Com($ck, f$)} $\to$ ($C, \delta$): Given function $f: \{0,1\}^l \rightarrow \mathbb{Z}_p$:
\begin{enumerate}
\item Construct matrix $T \in \mathbb{Z}_p^{2^{l/\iota} \times 2^{l - l/\iota}}$, where element $T_{ij} = f(\vec{b}^{(i,j)})$, $\vec{b}^{(i,j)} = (\vec{b}_{\text{head}}^{(j)}, \vec{b}_{\text{tail}}^{(i)})$ is defined as in Lemma 1.
\item For each row $\vec{T^{(i)}} \in \mathbb{Z}_p^{2^{l - l/\iota}}$ ($i \in [2^{l/\iota}]$), treat it as coefficients of a univariate polynomial to obtain $T^{(i)}(X)$, call the univariate commitment algorithm:
\[
(c_i, \delta_i) \gets \text{PC.Com}(\text{ck}_{\text{uni}}, T^{(i)}(X))
\]
where $\delta_i = (\vec{{\bm{T}}^{(i)}}, \vec{\eta}^{(i)})$ is the corresponding opening information.
\item Output commitment $C = (c_1, \ldots, c_{2^{l/\iota}})$ and opening information $\delta = ( \{ \delta_i \}_{i=1}^{2^{l/\iota}})$.
\end{enumerate}

\item \textbf{PC.Open($ck, C, f, \delta$)} $\to$ $b$: Verify that commitment $C$ corresponds to function $f$:
\begin{enumerate}
\item Reconstruct matrix $T$ from $f$, then obtain $T^{(i)}(X)$, $i \in [2^{l/\iota}]$.
\item For each $i \in [2^{l/\iota}]$, verify:
\[
\text{PC.Open}(\text{ck}_{\text{uni}}, c_i, T^{(i)}(X), \delta_i) \overset{?}{=} 1
\]
\item Output 1 if all verifications pass, otherwise output 0.
\end{enumerate}

\item \textbf{PC.Eval($ck,C, \vec{r}, f, \delta$)} $\to$ ($y, \pi_1,\pi_2$): Given commitment $C$, evaluation point $\vec{r} \in \mathbb{Z}_p^l$, function $f$ and opening information $\delta$,
both parties interactively execute:
\begin{enumerate}
\item Prover: 
\begin{itemize}
    \item Split $\vec{r}$ into $\vec{r}_{\text{head}}$ and $\vec{r}_{\text{tail}}$ as in Lemma 1.
    \item Construct vector $\vec{A} \in \mathbb{Z}_p^{2^{l/\iota}}$, where $A[i] = \chi_{\vec{b}_{\text{tail}}^{(i)}}(\vec{r}_{\text{tail}})$.
    \item Construct vector $\vec{B} \in \mathbb{Z}_p^{2^{l - l/\iota}}$, where $B[j] = \chi_{\vec{b}_{\text{head}}^{(j)}}(\vec{r}_{\text{head}})$.
    \item Compute $y=\tilde{f}(\vec{r}) = \langle \vec{A} \cdot T, \vec{B} \rangle =\langle \vec{u}, \vec{B} \rangle \in \mathbb{Z}_p$.
    \item Sample random vector $\vec{a} \overset{\$}{\leftarrow} \mathbb{Z}_p^{2^{l - l/\iota}}$, treat as coefficients of univariate polynomial to obtain $a(X)$, call the univariate polynomial commitment algorithm:
\[
(c_a, \delta_a) \gets \text{PC.Com}(\text{ck}_{\text{uni}}, a(X))
\]
where $\delta_a = (\vec{\bm{a}}, \vec{\eta_a})$
\item Compute $t = \langle \vec{a}, \vec{B} \rangle \in \mathbb{Z}_p$.
\item Send $(y,\pi_1)$,where $\pi_1=(c_a,t)$
\end{itemize}
\item Verifier sends random challenge $e \overset{\$}{\leftarrow} \mathbb{Z}_p$.
\item Prover: 
\begin{itemize}
\item Compute:
\[
\vec{z} = \vec{a} + e \cdot \vec{u} \in \mathbb{Z}_p^{2^{l - l/\iota}}
\]
\[
\vec{\eta}_z = \vec{\eta}_a + \text{R.Ecd}(e) \cdot \left( \sum_{i=1}^{2^{l/\iota}} A[i] \cdot \vec{\eta}^{(i)} \right)
\]
where $\vec{\eta}^{(i)}$ comes from $\delta_i$.
\item Compute $\delta_z = (\vec{\bm{z}}, \vec{\eta}_z)$, where $\vec{\bm{z}}$ as in univariate polynomial PC.Com steps 3-4.
\item Compute $c_z= \mathbf{A}_0 \cdot \vec{\bm{z}} + \mathbf{A}_1 \cdot \vec{\eta}_z$
\item Send $\pi_2=(\vec{z}, \delta_z,c_z)$
\end{itemize}
\end{enumerate} 
\item \textbf{PC.Verify($ck,C, \vec{r}, y, \pi_1,\pi_2$)} $\to$ $b$: Given commitment $C$, evaluation point $\vec{r}$, evaluation result $y$ and proof $\pi_1=(c_a,t)$, $\pi_2=(\vec{z}, \delta_z,c_z)$:
\begin{enumerate}
\item Verifier computes:
\begin{itemize}
\item Vectors $\vec{A}, \vec{B}$ as in PC.Eval steps 1-3.
\item $c_u = \sum_{i=1}^{2^{l/\iota}} \text{R.Ecd}(A[i]) \cdot c_i$.
\end{itemize}
\item Verifier checks:
\begin{itemize}
\item Linear relation: \[
 c_z \overset{?}{=} c_a + \text{R.Ecd}(e) \cdot c_u
\]
\item Dot product relation: \[
\langle \vec{z}, \vec{B} \rangle \overset{?}{=} t + e \cdot y
\]
\item Call univariate polynomial commitment algorithm: \[
PC.Open(ck_{uni},c_z,z(X),\delta_z) \overset{?}{=} 1
\]
\end{itemize}
\item Output 1 if all three checks pass, otherwise output 0.
\end{enumerate}
\end{itemize}